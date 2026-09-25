//! Fetching a signed price update from Pyth's Hermes, and posting it on-chain.
//!
//! # When this path is used, and what it costs
//!
//! `stock.rs` reads price accounts that already exist. That is the better
//! shape, and it is the one the adapter is configured for by default: Pyth
//! maintains a *sponsored* price account for some feeds, keeps it fresh, and
//! it carries **Full** verification — two thirds of the current guardian set.
//!
//! Sponsorship is per feed and per cluster, and it is not something we choose.
//! On devnet, an `Equity.US.<SYM>/USD` feed generally has no sponsored account
//! at all, so there is nothing to read and no amount of retrying produces one.
//!
//! The alternative is to bring the price yourself: Hermes will hand out a
//! signed update for any feed on request, and the receiver program will accept
//! it. There are two ways to post one, and they are not equivalent:
//!
//! - `post_update` verifies the Wormhole VAA in a *separate, earlier*
//!   transaction that writes an `encoded_vaa` account. The result is **Full**
//!   verification. It cannot be done in one transaction, so the price is posted
//!   in one slot and read in a later one.
//! - `post_update_atomic` carries the VAA inline and checks a subset of
//!   guardian signatures. It fits in one transaction — so the post and the
//!   refresh are atomic, and the adapter reads a price that cannot have moved
//!   or been replaced between the two. The result is **Partial** verification.
//!
//! This module implements the atomic one, because atomicity is the property
//! the adapter's whole design is about: the published value is derived from
//! inputs verified in the same transaction that publishes it.
//!
//! The cost is real and is not hidden: a partially verified update needs fewer
//! colluding guardians to forge than a fully verified one. A symbol cranked
//! this way must have `min_verification_level = 0` in its on-chain config,
//! which is exactly where a consumer can read it. A feed running at level 0 is
//! visibly a feed running at level 0, rather than one described as safe in a
//! keeper's configuration file that nobody else can see.

use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use pythnet_sdk::wire::{
    from_slice,
    v1::{AccumulatorUpdateData, MerklePriceUpdate, Proof},
};

/// Pyth's public Hermes. Override for a self-hosted one.
pub const DEFAULT_HERMES: &str = "https://hermes.pyth.network";

/// `sha256("global:post_update_atomic")[..8]`, the receiver's instruction
/// discriminator. Anchor derives it from the instruction name, so it is fixed
/// by the program's source rather than by anything we choose.
pub const IX_POST_UPDATE_ATOMIC: [u8; 8] = [49, 172, 84, 192, 175, 180, 52, 234];

/// Wormhole's Solana receiver, which owns the guardian set accounts.
pub const WORMHOLE_PROGRAM: Pubkey =
    Pubkey::from_str_const("worm2ZoG2kUd4vFXhvjh93UUH596ayRfgQ2MgjNMTth");

/// Which treasury account pays for the update. Any u8 is valid; the receiver
/// has one per value to spread the write load, and 0 is as good as any.
pub const TREASURY_ID: u8 = 0;

/// How many guardian signatures to keep when posting atomically.
///
/// A full VAA carries a signature from every guardian that signed, which is
/// about a kilobyte. Two of those plus a refresh does not fit in Solana's
/// 1232-byte transaction limit, so the signature list is trimmed — which is
/// the whole reason `post_update_atomic` produces a *Partial* verification
/// rather than a Full one.
///
/// Five is Pyth's own default for this, and it is the number that ends up in
/// the price account as `VerificationLevel::Partial { num_signatures }`, where
/// a consumer can read it.
pub const KEPT_SIGNATURES: u8 = 5;

/// Offsets in a VAA header: version (1 byte), guardian set index (4 bytes
/// big-endian), signature count (1 byte), then that many 66-byte signatures.
const VAA_HEADER_LEN: usize = 6;
const VAA_SIGNATURE_LEN: usize = 66;

/// Drop all but the first `keep` guardian signatures from a VAA.
///
/// The receiver verifies whichever signatures are present and records how many
/// there were, so trimming does not make the update invalid — it makes it
/// *less* verified, visibly, in a field the adapter checks against each
/// symbol's configured minimum.
///
/// Returns the VAA unchanged if it already carries no more than `keep`.
pub fn trim_signatures(vaa: &[u8], keep: u8) -> Result<Vec<u8>> {
    if keep == 0 {
        bail!("refusing to build a VAA with no guardian signatures at all");
    }
    let count = *vaa
        .get(5)
        .ok_or_else(|| anyhow!("VAA is {} bytes, too short for a header", vaa.len()))?;
    if count <= keep {
        return Ok(vaa.to_vec());
    }

    let sigs_end = VAA_HEADER_LEN + usize::from(count) * VAA_SIGNATURE_LEN;
    let body = vaa
        .get(sigs_end..)
        .ok_or_else(|| anyhow!("VAA claims {count} signatures but is only {} bytes", vaa.len()))?;

    let mut out = Vec::with_capacity(VAA_HEADER_LEN + usize::from(keep) * VAA_SIGNATURE_LEN + body.len());
    out.extend_from_slice(&vaa[..5]);
    out.push(keep);
    out.extend_from_slice(&vaa[VAA_HEADER_LEN..VAA_HEADER_LEN + usize::from(keep) * VAA_SIGNATURE_LEN]);
    out.extend_from_slice(body);
    Ok(out)
}

/// One feed's signed update, ready to be posted.
#[derive(Debug, Clone, PartialEq)]
pub struct SignedUpdate {
    /// The Wormhole VAA attesting to the merkle root.
    pub vaa: Vec<u8>,
    /// This feed's leaf and its proof against that root.
    pub merkle_update: MerklePriceUpdate,
    /// Guardian set that signed the VAA, read out of its header. Needed to
    /// name the right guardian set account, which the receiver checks.
    pub guardian_set_index: u32,
}

/// Parse one Hermes `binary.data` entry into the pieces `post_update_atomic`
/// needs, keeping only the requested feed's leaf.
///
/// Hermes returns an accumulator update covering every feed you asked about,
/// under a single VAA. Posting is per feed, so the caller asks for one feed at
/// a time and this returns that feed's leaf — but Hermes is free to return
/// more than was asked for, so which leaf is which is checked rather than
/// assumed.
pub fn parse_update(blob: &[u8], want_feed_id: &[u8; 32]) -> Result<SignedUpdate> {
    let data = AccumulatorUpdateData::try_from_slice(blob)
        .map_err(|e| anyhow!("Hermes returned bytes that are not an accumulator update: {e:?}"))?;

    let Proof::WormholeMerkle { vaa, updates } = data.proof;
    let vaa: Vec<u8> = vaa.into();

    if updates.is_empty() {
        bail!("Hermes returned an update with no price leaves in it");
    }

    let guardian_set_index = guardian_set_index(&vaa)?;

    for update in updates {
        let message: Vec<u8> = update.message.clone().into();
        match feed_id_of(&message) {
            Some(id) if id == *want_feed_id => {
                return Ok(SignedUpdate {
                    vaa,
                    merkle_update: update,
                    guardian_set_index,
                })
            }
            _ => continue,
        }
    }

    bail!(
        "Hermes returned {} leaf/leaves, none of them for feed {}",
        1,
        hex::encode(want_feed_id)
    )
}

/// The feed id of a price message, without trusting its position.
///
/// A message is a one-byte discriminant followed by the body, and for a
/// `PriceFeedMessage` (discriminant 0) the body begins with the 32-byte feed
/// id. Anything else — a TWAP message, a stake-caps message — is not ours.
fn feed_id_of(message: &[u8]) -> Option<[u8; 32]> {
    let msg: pythnet_sdk::messages::Message = from_slice::<byteorder::BE, _>(message).ok()?;
    match msg {
        pythnet_sdk::messages::Message::PriceFeedMessage(m) => Some(m.feed_id),
        _ => None,
    }
}

/// Read the guardian set index out of a VAA header.
///
/// Layout: version (1 byte), then the guardian set index as 4 bytes
/// big-endian. Naming the wrong guardian set account fails the transaction
/// with a constraint error that does not say which account was wrong, so this
/// is read from the VAA rather than assumed to be the current set.
fn guardian_set_index(vaa: &[u8]) -> Result<u32> {
    let bytes: [u8; 4] = vaa
        .get(1..5)
        .ok_or_else(|| anyhow!("VAA is {} bytes, too short for a header", vaa.len()))?
        .try_into()
        .expect("checked length above");
    Ok(u32::from_be_bytes(bytes))
}

/// The guardian set account for a given index.
pub fn guardian_set_address(index: u32) -> Pubkey {
    Pubkey::find_program_address(
        &[b"GuardianSet", &index.to_be_bytes()],
        &WORMHOLE_PROGRAM,
    )
    .0
}

/// Build the `post_update_atomic` instruction.
///
/// `price_update` is a fresh keypair the caller signs with: the receiver
/// creates the account, and creating it fresh each time is what keeps the post
/// and the refresh in one transaction — an existing account would have to
/// belong to this write authority already.
pub fn post_update_atomic_ix(
    update: &SignedUpdate,
    payer: &Pubkey,
    price_update: &Pubkey,
) -> Result<Instruction> {
    let mut data = IX_POST_UPDATE_ATOMIC.to_vec();
    let params = pyth_solana_receiver_sdk::PostUpdateAtomicParams {
        vaa: update.vaa.clone(),
        merkle_price_update: update.merkle_update.clone(),
        treasury_id: TREASURY_ID,
    };
    borsh::BorshSerialize::serialize(&params, &mut data)
        .context("serialising PostUpdateAtomicParams")?;

    Ok(Instruction {
        program_id: pyth_solana_receiver_sdk::ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(guardian_set_address(update.guardian_set_index), false),
            AccountMeta::new_readonly(pyth_solana_receiver_sdk::pda::get_config_address(), false),
            AccountMeta::new(
                pyth_solana_receiver_sdk::pda::get_treasury_address(TREASURY_ID),
                false,
            ),
            AccountMeta::new(*price_update, true),
            AccountMeta::new_readonly(anchor_lang::solana_program::system_program::ID, false),
            AccountMeta::new_readonly(*payer, true),
        ],
        data,
    })
}

/// Ask Hermes for the latest signed update for one feed.
/// One client for every Hermes request, carrying an agent that names the tool.
///
/// `reqwest::get` builds a fresh client per call, which throws away connection
/// reuse, and sends whatever default agent the build happens to have. The CDN
/// in front of Hermes refuses some of those outright, and the refusal arrives
/// as a bare 403 that looks like a bad feed id rather than a rejected client.
/// Turn a refused Hermes response into the thing to do about it.
///
/// These three statuses mean different things and the difference decides the
/// next step, but a bare "HTTP status client error" reads identically for all
/// of them -- and, arriving in a loop next to a feed id, reads like the feed id
/// is wrong when it is not.
fn describe_refusal(status: reqwest::StatusCode, base: &str) -> String {
    match status.as_u16() {
        401 => format!(
            "{base} answered 401 Unauthorized: this endpoint requires credentials. \
             The request itself is fine and no feed id will change that. Either set \
             HERMES_API_KEY for a key this endpoint accepts, or point the keeper at \
             one that serves you: --hermes https://your-hermes. Pyth's public endpoint \
             has been open historically; if it is refusing you now, it is gated or \
             moved, and a provider-hosted or self-run Hermes is the way through."
        ),
        403 => format!(
            "{base} answered 403 Forbidden: the request reached the service and was \
             rejected. Usually a CDN refusing the client rather than anything about \
             the feed. Check whether a proxy sits in front of this machine."
        ),
        429 => format!(
            "{base} answered 429 Too Many Requests: slow the crank down with \
             --interval, or use an endpoint with a higher allowance."
        ),
        _ => format!("{base} answered {status}"),
    }
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!(
                "svi-keeper/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/princedotrs/solana-valuation-interface)"
            ))
            .timeout(std::time::Duration::from_secs(20))
            .build()
            // Only fails if the TLS backend cannot start, which is not
            // recoverable and not worth propagating through every call site.
            .expect("building the HTTP client")
    })
}

pub async fn fetch(base: &str, feed_id: &[u8; 32]) -> Result<SignedUpdate> {
    let url = format!(
        "{}/v2/updates/price/latest?ids[]={}&encoding=base64",
        base.trim_end_matches('/'),
        hex::encode(feed_id)
    );

    let mut req = http_client().get(&url);
    // Some Hermes deployments are gated. An endpoint that needs a key is a
    // deployment choice, not a property of the protocol, so the key is read
    // from the environment and never from a file that could be committed.
    if let Ok(key) = std::env::var("HERMES_API_KEY") {
        if !key.trim().is_empty() {
            req = req.bearer_auth(key.trim());
        }
    }

    let resp = req
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;

    let status = resp.status();
    if !status.is_success() {
        bail!("{}", describe_refusal(status, base));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .context("Hermes returned a body that is not JSON")?;

    let encoded = body
        .get("binary")
        .and_then(|b| b.get("data"))
        .and_then(serde_json::Value::as_array)
        .and_then(|a| a.first())
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            anyhow!(
                "Hermes response has no binary.data[0]. Body starts: {}",
                &body.to_string().chars().take(200).collect::<String>()
            )
        })?;

    let blob = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("binary.data[0] is not base64")?;

    parse_update(&blob, feed_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sha256_hasher::hashv;

    /// A VAA with `n` signatures, each 66 bytes, and a recognisable body.
    fn fake_vaa(n: u8) -> Vec<u8> {
        let mut v = vec![1, 0, 0, 0, 7, n];
        for i in 0..n {
            v.extend(std::iter::repeat(i).take(VAA_SIGNATURE_LEN));
        }
        v.extend_from_slice(b"BODY");
        v
    }

    #[test]
    fn trimming_keeps_the_header_the_first_n_signatures_and_the_whole_body() {
        let trimmed = trim_signatures(&fake_vaa(13), 5).unwrap();
        assert_eq!(trimmed[..5], [1, 0, 0, 0, 7], "version and set index survive");
        assert_eq!(trimmed[5], 5, "the count is rewritten, not just the list");
        assert_eq!(trimmed.len(), VAA_HEADER_LEN + 5 * VAA_SIGNATURE_LEN + 4);
        assert_eq!(&trimmed[trimmed.len() - 4..], b"BODY", "the body is intact");
        // The kept signatures are the first five, in order.
        for i in 0..5u8 {
            let at = VAA_HEADER_LEN + usize::from(i) * VAA_SIGNATURE_LEN;
            assert_eq!(trimmed[at], i);
        }
        // Still parses as the same guardian set.
        assert_eq!(guardian_set_index(&trimmed).unwrap(), 7);
    }

    #[test]
    fn trimming_is_a_no_op_when_there_is_nothing_to_trim() {
        let v = fake_vaa(3);
        assert_eq!(trim_signatures(&v, 5).unwrap(), v);
        assert_eq!(trim_signatures(&v, 3).unwrap(), v);
    }

    #[test]
    fn trimming_refuses_the_degenerate_and_the_malformed() {
        // Zero signatures would be an unverified update that still looks like
        // an update.
        assert!(trim_signatures(&fake_vaa(13), 0).is_err());
        // A VAA whose claimed count runs past its own bytes.
        assert!(trim_signatures(&[1, 0, 0, 0, 7, 200, 0, 0], 5).is_err());
        assert!(trim_signatures(&[1, 0], 5).is_err());
    }

    #[test]
    fn two_trimmed_vaas_leave_room_for_the_rest_of_the_transaction() {
        // The reason trimming exists. Solana's packet limit is 1232 bytes and
        // a crank posts two updates; if this assertion ever fails, the fix is
        // fewer signatures or fewer instructions, not a bigger number here.
        let one = trim_signatures(&fake_vaa(19), KEPT_SIGNATURES).unwrap().len();
        assert!(2 * one < 900, "two trimmed VAAs are {} bytes", 2 * one);
    }

    #[test]
    fn guardian_set_index_is_read_big_endian_from_the_header() {
        // version 1, then set index 4 as four big-endian bytes.
        let vaa = vec![1, 0, 0, 0, 4, 0xde, 0xad];
        assert_eq!(guardian_set_index(&vaa).unwrap(), 4);

        // A large index must not be truncated or read the wrong way round.
        let vaa = vec![1, 0x00, 0x01, 0x02, 0x03];
        assert_eq!(guardian_set_index(&vaa).unwrap(), 0x0001_0203);
    }

    #[test]
    fn a_truncated_vaa_is_an_error_not_a_zero() {
        // Defaulting to set 0 here would build a transaction naming a real but
        // wrong account, which fails with a constraint error that names no
        // cause. Better to stop while we still know why.
        assert!(guardian_set_index(&[1, 0, 0]).is_err());
        assert!(guardian_set_index(&[]).is_err());
    }

    #[test]
    fn guardian_set_addresses_differ_per_index_and_are_pdas() {
        let a = guardian_set_address(3);
        let b = guardian_set_address(4);
        assert_ne!(a, b);
        assert!(!a.is_on_curve(), "a guardian set account is a PDA");
    }

    #[test]
    fn garbage_is_rejected_with_a_message_naming_hermes() {
        let err = parse_update(b"not an accumulator update", &[0u8; 32])
            .unwrap_err()
            .to_string();
        assert!(err.contains("Hermes"), "{err}");
    }

    #[test]
    fn the_instruction_account_order_matches_the_receiver_sdk() {
        // Order and signer/writable flags are copied from
        // pyth_solana_receiver_sdk::cpi::accounts::PostUpdateAtomic. Getting
        // one wrong produces a constraint failure that names no account.
        let update = SignedUpdate {
            vaa: vec![1, 0, 0, 0, 2],
            merkle_update: MerklePriceUpdate {
                message: vec![0u8; 8].into(),
                proof: Default::default(),
            },
            guardian_set_index: 2,
        };
        let payer = Pubkey::new_unique();
        let price = Pubkey::new_unique();
        let ix = post_update_atomic_ix(&update, &payer, &price).unwrap();

        assert_eq!(ix.program_id, pyth_solana_receiver_sdk::ID);
        assert_eq!(ix.accounts.len(), 7);
        assert_eq!(ix.data[..8], IX_POST_UPDATE_ATOMIC);

        assert_eq!(ix.accounts[0].pubkey, payer);
        assert!(ix.accounts[0].is_signer && ix.accounts[0].is_writable);
        assert_eq!(ix.accounts[1].pubkey, guardian_set_address(2));
        assert!(!ix.accounts[1].is_writable);
        assert_eq!(
            ix.accounts[2].pubkey,
            pyth_solana_receiver_sdk::pda::get_config_address()
        );
        assert!(ix.accounts[3].is_writable, "the treasury is paid");
        assert_eq!(ix.accounts[4].pubkey, price);
        assert!(
            ix.accounts[4].is_signer && ix.accounts[4].is_writable,
            "the price account is created, so it signs"
        );
        assert_eq!(
            ix.accounts[5].pubkey,
            anchor_lang::solana_program::system_program::ID
        );
        assert_eq!(ix.accounts[6].pubkey, payer, "payer is the write authority");
        assert!(ix.accounts[6].is_signer);
    }

    #[test]
    fn the_discriminator_is_the_anchor_one_for_this_instruction_name() {
        let want = hashv(&[b"global:post_update_atomic"]).to_bytes();
        assert_eq!(&want[..8], &IX_POST_UPDATE_ATOMIC);
    }
}

#[cfg(test)]
mod refusal_tests {
    use super::describe_refusal;
    use reqwest::StatusCode;

    const BASE: &str = "https://hermes.pyth.network";

    /// 401 is the one that wasted a debugging session. It arrived as "HTTP
    /// status client error" beside a feed id, once every ten seconds, which
    /// reads like the feed id is wrong. It is not: the request never got far
    /// enough for the feed id to matter.
    #[test]
    fn unauthorized_says_credentials_and_rules_out_the_feed() {
        let m = describe_refusal(StatusCode::UNAUTHORIZED, BASE);
        assert!(m.contains("401"), "{m}");
        assert!(m.contains("HERMES_API_KEY"), "must name the way to supply one: {m}");
        assert!(m.contains("--hermes"), "must name the way to change endpoint: {m}");
        assert!(m.contains("no feed id will change that"), "must rule out the feed: {m}");
    }

    #[test]
    fn forbidden_and_unauthorized_do_not_give_the_same_advice() {
        let unauth = describe_refusal(StatusCode::UNAUTHORIZED, BASE);
        let forbid = describe_refusal(StatusCode::FORBIDDEN, BASE);
        assert_ne!(unauth, forbid, "401 and 403 need different responses");
        assert!(forbid.contains("403"), "{forbid}");
        assert!(!forbid.contains("HERMES_API_KEY"), "a key does not fix a 403: {forbid}");
    }

    #[test]
    fn rate_limiting_points_at_the_interval() {
        let m = describe_refusal(StatusCode::TOO_MANY_REQUESTS, BASE);
        assert!(m.contains("429") && m.contains("--interval"), "{m}");
    }

    /// An unmapped status must still name the endpoint and the code rather
    /// than swallowing them.
    #[test]
    fn an_unmapped_status_still_reports_itself() {
        let m = describe_refusal(StatusCode::INTERNAL_SERVER_ERROR, BASE);
        assert!(m.contains("500"), "{m}");
        assert!(m.contains(BASE), "{m}");
    }

    /// The endpoint is always named, because the whole question on a refusal
    /// is which endpoint refused.
    #[test]
    fn every_message_names_the_endpoint() {
        for s in [
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::BAD_GATEWAY,
        ] {
            let m = describe_refusal(s, "https://my-hermes.example");
            assert!(m.contains("https://my-hermes.example"), "{s}: {m}");
        }
    }
}

//! End-to-end: initialize a feed, refresh it, and read the published quote.
//!
//! This is the step that turns "the program compiles" into "the program
//! works". It runs the whole chain for real — adapter reads Hylo's accounts,
//! computes NAV with `hylo-core`, CPIs into `svi-core`, and `svi-core` writes
//! a quote account — then reads that account back and checks the value against
//! an independent off-chain computation over the same state.
//!
//! Point it at a local validator started by `scripts/localnet.sh`, which
//! clones Hylo's real mainnet accounts. Nothing here is mocked: the same bytes
//! mainnet has, executed by the same program that would run on mainnet.
//!
//! Usage:
//!     cargo run -- [--rpc URL] [--keypair PATH]

use std::env;
use std::sync::Arc;

use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};
use anyhow::{anyhow, bail, Context, Result};
use solana_commitment_config::CommitmentConfig;
use solana_keypair::{read_keypair_file, Keypair};
use solana_message::Message;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sha256_hasher::hashv;
use solana_signer::Signer;
use solana_transaction::Transaction;

/// Deployed program ids. Must match `declare_id!` in each program.
///
/// Parsed at startup rather than via a `pubkey!` macro, whose path moves
/// between Anchor versions — and this tool has to keep working across them.
const SVI_CORE_ID: &str = "H6495aW1Cxyoz2R7MuYVHF3Pu3FumFE9cZwKSJCR8oHH";
const ADAPTER_ID: &str = "FfK5xyE3v3GT2F9wzqhpLMHx7zCJsGPQrHAvxtgL3Mpr";

/// Anchor discriminators, sha256("global:<name>")[..8].
const IX_INITIALIZE_FEED: [u8; 8] = [167, 251, 140, 58, 66, 138, 187, 95];
const IX_ADAPTER_INIT: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
const IX_REFRESH: [u8; 8] = [133, 61, 124, 93, 60, 229, 233, 255];

const VALUE_TYPE_PROTOCOL_NAV: u8 = 1;
const USD: u16 = 840;
const XSOL_DECIMALS: u8 = 6;
const QUOTE_DECIMALS: u8 = 9;
/// Generous for a test: mainnet state cloned into a local validator is, by
/// definition, older than the local clock.
const MAX_AGE_SLOTS: u64 = 1_000_000;
const MAX_BAND_BPS: u64 = 500;

/// The 320-byte quote account, read back by offset. Consumers parse it exactly
/// this way, so decoding it here also exercises the layout contract.
#[derive(Debug)]
struct Quote {
    base_amount: u64,
    quote_amount: u64,
    lower: u64,
    upper: u64,
    observed_slot: u64,
    valid_until_slot: u64,
    sequence: u64,
    status_flags: u64,
}

/// A `Quote` account is 8 discriminator bytes plus a 320-byte payload, and the
/// layout is frozen (`svi-core/tests/core.rs::layout_is_frozen`). Anything
/// shorter is not a partially-written quote we could read the front of — it is
/// a different account, and decoding it would produce numbers with no meaning.
const QUOTE_PAYLOAD_LEN: usize = 320;

impl Quote {
    /// Offsets are asserted in `svi-core/tests/core.rs::layout_is_frozen`.
    fn decode(data: &[u8]) -> Result<Self> {
        let d = data
            .get(8..)
            .ok_or_else(|| anyhow!("quote account too small for its discriminator"))?;
        if d.len() < QUOTE_PAYLOAD_LEN {
            bail!("quote payload is {} bytes, expected {QUOTE_PAYLOAD_LEN}", d.len());
        }
        let u64_at = |o: usize| -> u64 {
            u64::from_le_bytes(d[o..o + 8].try_into().expect("checked length above"))
        };
        Ok(Quote {
            base_amount: u64_at(224),
            quote_amount: u64_at(232),
            lower: u64_at(240),
            upper: u64_at(248),
            observed_slot: u64_at(256),
            valid_until_slot: u64_at(280),
            sequence: u64_at(288),
            status_flags: u64_at(296),
        })
    }
}

fn feed_id() -> [u8; 32] {
    hashv(&[b"hylo-xsol-nav-v1"]).to_bytes()
}

fn arg(flag: &str) -> Option<String> {
    let a: Vec<String> = env::args().collect();
    a.iter().position(|x| x == flag).and_then(|i| a.get(i + 1).cloned())
}

async fn send(
    rpc: &RpcClient,
    payer: &Keypair,
    ix: Instruction,
    label: &str,
) -> Result<()> {
    let bh = rpc.get_latest_blockhash().await.context("blockhash")?;
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[payer], msg, bh);
    match rpc.send_and_confirm_transaction(&tx).await {
        Ok(sig) => {
            println!("  {label:<22} ok   {sig}");
            Ok(())
        }
        Err(e) => {
            // Print the program logs: an adapter refusal is a *correct*
            // outcome in some states, and the error code says which.
            println!("  {label:<22} FAILED");
            println!("      {e}");
            Err(anyhow!("{label} failed"))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let url = arg("--rpc").unwrap_or_else(|| "http://127.0.0.1:8899".to_string());
    let kp_path = arg("--keypair").unwrap_or_else(|| {
        format!("{}/.config/solana/id.json", env::var("HOME").unwrap_or_default())
    });

    println!("svi-e2e — publish a real quote and read it back\n");
    println!("  rpc      {url}");
    println!("  keypair  {kp_path}\n");

    let svi_core: Pubkey = SVI_CORE_ID.parse().expect("svi-core id is a valid pubkey");
    let adapter: Pubkey = ADAPTER_ID.parse().expect("adapter id is a valid pubkey");

    let payer = read_keypair_file(&kp_path)
        .map_err(|e| anyhow!("cannot read keypair at {kp_path}: {e}"))?;
    let rpc = RpcClient::new_with_commitment(url.clone(), CommitmentConfig::confirmed());

    let fid = feed_id();
    let (descriptor, _) = Pubkey::find_program_address(&[b"descriptor", &fid], &svi_core);
    let (quote, _) = Pubkey::find_program_address(&[b"quote", &fid], &svi_core);
    let (config, _) = Pubkey::find_program_address(&[b"config"], &adapter);
    let (adapter_authority, _) = Pubkey::find_program_address(&[b"authority"], &adapter);

    println!("  descriptor        {descriptor}");
    println!("  quote             {quote}");
    println!("  adapter authority {adapter_authority}\n");

    // This tool deploys nothing, and a fresh Surfnet has neither program. An
    // address Surfpool cannot find on mainnet becomes an empty owned-by-system
    // account rather than a missing one, so the failure surfaces from inside
    // simulation as "This program may not be used for executing instructions"
    // -- which reads like a bad instruction, not a missing deploy. Check it
    // here so the message names the actual problem.
    for (name, id, dir, so) in [
        ("svi-core", svi_core, "programs/svi-core", "svi_core"),
        ("svi-hylo-adapter", adapter, "programs/svi-hylo-adapter", "svi_hylo_adapter"),
    ] {
        let executable = rpc.get_account(&id).await.map(|a| a.executable).unwrap_or(false);
        if !executable {
            bail!(
                "{name} ({id}) is not deployed on this validator.\n\n\
                 Deploy with the runbook, which deploys both programs and then\n\
                 publishes -- making this tool unnecessary unless you want to\n\
                 bypass txtx:\n\n  \
                 cd programs/svi-hylo-adapter\n  \
                 surfpool run publish --env localnet --unsupervised\n\n\
                 Do not reach for `solana program deploy` against a Surfnet. The\n\
                 CLI wants TPU and gossip, which an RPC-only SVM does not serve,\n\
                 and it sends loader instructions Surfpool rejects:\n  \
                 `Failed find any cluster node info for upcoming leaders`\n  \
                 `Account allocation failed: ... invalid instruction data`\n\
                 The second one leaves the old binary in place and running, so a\n\
                 failed redeploy looks exactly like a code change that did nothing.\n\n\
                 Expected artifacts: {dir}/target/deploy/{so}.so"
            );
        }
    }
    println!("  both programs deployed and executable\n");

    // The accounts the adapter reads, taken from the SDK rather than
    // hardcoded, so this cannot drift from what the program expects.
    let hylo_accounts = hylo_quotes::protocol_state::ProtocolAccounts::lst_pubkeys();
    let (hylo_state, xsol_mint, sol_usd, _clock) =
        (hylo_accounts[0], hylo_accounts[1], hylo_accounts[2], hylo_accounts[3]);

    for (name, pk) in [("hylo state", hylo_state), ("xSOL mint", xsol_mint), ("pyth SOL/USD", sol_usd)] {
        if rpc.get_account(&pk).await.is_err() {
            bail!(
                "{name} ({pk}) is not present on this validator.\n\
                 Start it with scripts/localnet.sh, which clones these from mainnet."
            );
        }
    }
    println!("  all three Hylo accounts present on this validator\n");

    // ---- 1. create the feed -------------------------------------------
    let mut data = IX_INITIALIZE_FEED.to_vec();
    data.extend_from_slice(&fid);
    data.extend_from_slice(adapter.as_ref());
    data.extend_from_slice(adapter_authority.as_ref());
    data.extend_from_slice(config.as_ref());
    data.extend_from_slice(xsol_mint.as_ref());
    data.extend_from_slice(Pubkey::default().as_ref()); // quote_mint: fiat
    data.extend_from_slice(&[0u8; 32]); // methodology_hash, unfrozen in a test
    data.extend_from_slice(&MAX_AGE_SLOTS.to_le_bytes());
    data.extend_from_slice(&USD.to_le_bytes());
    data.push(VALUE_TYPE_PROTOCOL_NAV);
    data.push(XSOL_DECIMALS);
    data.push(QUOTE_DECIMALS);

    if rpc.get_account(&quote).await.is_err() {
        send(&rpc, &payer, Instruction {
            program_id: svi_core,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(descriptor, false),
                AccountMeta::new(quote, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data,
        }, "initialize_feed").await?;
    } else {
        println!("  initialize_feed        skipped (feed already exists)");
    }

    // ---- 2. create the adapter config ----------------------------------
    let mut data = IX_ADAPTER_INIT.to_vec();
    data.extend_from_slice(svi_core.as_ref());
    data.extend_from_slice(descriptor.as_ref());
    data.extend_from_slice(quote.as_ref());
    data.extend_from_slice(&[0u8; 32]); // methodology_hash
    data.extend_from_slice(&[0u8; 20]); // sdk_revision
    data.extend_from_slice(&MAX_BAND_BPS.to_le_bytes());

    if rpc.get_account(&config).await.is_err() {
        send(&rpc, &payer, Instruction {
            program_id: adapter,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(adapter_authority, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data,
        }, "adapter initialize").await?;
    } else {
        println!("  adapter initialize     skipped (config already exists)");
    }

    // ---- 3. the actual thing -------------------------------------------
    println!();
    send(&rpc, &payer, Instruction {
        program_id: adapter,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new_readonly(hylo_state, false),
            AccountMeta::new_readonly(xsol_mint, false),
            AccountMeta::new_readonly(sol_usd, false),
            AccountMeta::new_readonly(descriptor, false),
            AccountMeta::new(quote, false),
            AccountMeta::new_readonly(adapter_authority, false),
            AccountMeta::new_readonly(svi_core, false),
        ],
        data: IX_REFRESH.to_vec(),
    }, "refresh_xsol_nav").await?;

    // ---- 4. read it back ------------------------------------------------
    let acct = rpc.get_account(&quote).await.context("reading quote account")?;
    let q = Quote::decode(&acct.data)?;

    let render = |bits: u64| format!("{}.{:09}", bits / 1_000_000_000, bits % 1_000_000_000);

    println!("\n  PUBLISHED QUOTE  (read back from {quote})");
    println!("  ─────────────────────────────────────────────────────────");
    println!("    quote_amount   $ {}", render(q.quote_amount));
    println!("    lower / upper  $ {} .. $ {}", render(q.lower), render(q.upper));
    println!("    base_amount      {} (1 whole xSOL)", q.base_amount);
    println!("    observed_slot    {}", q.observed_slot);
    println!("    valid_until      {}", q.valid_until_slot);
    println!("    sequence         {}", q.sequence);
    println!("    status_flags     {:#b}", q.status_flags);

    // ---- 5. check it against an independent computation ------------------
    let provider = hylo_quotes::protocol_state::RpcStateProvider::new(Arc::new(
        RpcClient::new_with_commitment(url, CommitmentConfig::confirmed()),
    ));
    match provider.fetch_lst_context().await {
        Ok(ctx) => {
            use hylo_core::exchange_context::ExchangeContext;
            let redeem = ctx.levercoin_redeem_nav().context("offchain redeem nav")?;
            println!("\n  independently recomputed off-chain: $ {redeem}");
            if redeem.bits == q.quote_amount {
                println!("  MATCH — the on-chain program and an off-chain reader agree exactly.");
            } else {
                println!(
                    "  DIFFERS — on-chain {} vs off-chain {}.",
                    q.quote_amount, redeem.bits
                );
                println!("  Expected if state moved between the two reads; investigate if static.");
            }
        }
        Err(e) => println!("\n  (off-chain cross-check unavailable: {e})"),
    }

    println!("\n  Anyone can read that account. No API, no permission.");
    Ok(())
}

//! Live end-to-end test against a Surfnet: deploy, initialize, refresh, and
//! read back a real xSOL NAV quote computed from Hylo's real mainnet state.
//!
//!     surfpool start          # any time; a long-running one is fine
//!     cargo test --test surfnet -- --nocapture
//!
//! Self-sufficient by design, because every manual step it replaces has
//! already been botched at least once in this project's history:
//!
//! * Deploys BOTH programs itself through `surfnet_setAccount`, the same
//!   cheatcode txtx's `svm::deploy_program` uses. This is the only deploy path
//!   that works against a Surfnet (`solana program deploy` wants TPU/gossip),
//!   and doing it every run guarantees the binary under test is the one just
//!   built -- a failed redeploy is otherwise indistinguishable from a code
//!   change that did nothing.
//! * Re-snapshots Hylo's three accounts from mainnet before refreshing. A
//!   Surfnet clones each account once and never updates it; Pyth prices move
//!   by pushed transactions and a local fork has none, so the oracle ages out
//!   of Hylo's window within about a minute of `surfpool start`. Re-cloning is
//!   what a fresh fork does, nothing more: no byte is edited.
//! * Moves the Surfnet clock forward to the snapshot's slot if it lags, since
//!   hylo-core requires `posted_slot <= clock.slot`. The clock can only go
//!   forward, so a Surfnet that has run AHEAD of mainnet cannot be fixed and
//!   the test says so.
//! * Skips `initialize_feed` / `initialize` when their PDAs already exist.
//!
//! Skips with a loud message when no Surfnet is reachable, so plain
//! `cargo test` stays green offline.
//!
//! Environment:
//!     SVI_SURFNET_RPC   default http://127.0.0.1:8899
//!     SVI_MAINNET_RPC   default https://api.mainnet-beta.solana.com
//!     SVI_KEYPAIR       default ~/.config/solana/id.json

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};
use anchor_lang::AccountDeserialize;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;
use solana_commitment_config::CommitmentConfig;
use solana_keypair::{read_keypair_file, Keypair};
use solana_message::Message;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::request::RpcRequest;
use solana_sha256_hasher::hashv;
use solana_signer::Signer;
use solana_transaction::Transaction;

use hylo_core::exchange_context::ExchangeContext;
use hylo_idl::exchange::accounts::Hylo;
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;

const SVI_CORE_ID: &str = "H6495aW1Cxyoz2R7MuYVHF3Pu3FumFE9cZwKSJCR8oHH";
const ADAPTER_ID: &str = "FfK5xyE3v3GT2F9wzqhpLMHx7zCJsGPQrHAvxtgL3Mpr";
const BPF_LOADER_UPGRADEABLE: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const CLOCK_SYSVAR: &str = "SysvarC1ock11111111111111111111111111111111";
const SLOTS_PER_EPOCH: u64 = 432_000;

/// sha256("global:<name>")[..8]. Cross-checked in tools/txtx-encode-check.
const IX_INITIALIZE_FEED: [u8; 8] = [167, 251, 140, 58, 66, 138, 187, 95];
const IX_ADAPTER_INIT: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
const IX_REFRESH: [u8; 8] = [133, 61, 124, 93, 60, 229, 233, 255];

const VALUE_TYPE_PROTOCOL_NAV: u8 = 1;
const USD: u16 = 840;
const XSOL_DECIMALS: u8 = 6;
const QUOTE_DECIMALS: u8 = 9;
const MAX_AGE_SLOTS: u64 = 750;
const MAX_BAND_BPS: u64 = 500;
const SDK_REVISION: [u8; 20] = [
    238, 141, 28, 186, 2, 58, 226, 109, 94, 129, 199, 124, 202, 181, 42, 79, 113, 196, 74, 63,
];

fn pk(s: &str) -> Pubkey {
    s.parse().expect("hardcoded pubkey")
}

fn feed_id() -> [u8; 32] {
    hashv(&[b"hylo-xsol-nav-v1"]).to_bytes()
}

/// Never print an RPC URL: it may carry an API key, and test output ends up
/// in screenshots.
fn redact(url: &str) -> String {
    match url.find('?') {
        Some(i) => format!("{}?<redacted>", &url[..i]),
        None => url.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Surfnet cheatcodes
// ---------------------------------------------------------------------------

async fn set_account(
    rpc: &RpcClient,
    key: &Pubkey,
    lamports: u64,
    data: &[u8],
    owner: &Pubkey,
    executable: bool,
) -> Result<()> {
    rpc.send::<serde_json::Value>(
        RpcRequest::Custom { method: "surfnet_setAccount" },
        json!([
            key.to_string(),
            {
                "lamports": lamports,
                "data": hex::encode(data),
                "owner": owner.to_string(),
                "executable": executable,
                "rent_epoch": 0u64,
            }
        ]),
    )
    .await
    .with_context(|| format!("surfnet_setAccount {key}"))?;
    Ok(())
}

/// Same account shapes txtx's `cheatcode_deploy_program` writes. Layouts are
/// `UpgradeableLoaderState`, bincode-encoded: a u32 variant tag, then fields.
async fn deploy(rpc: &RpcClient, program_id: &Pubkey, so: &[u8], authority: &Pubkey) -> Result<()> {
    let loader = pk(BPF_LOADER_UPGRADEABLE);
    let (programdata, _) = Pubkey::find_program_address(&[program_id.as_ref()], &loader);
    let slot = rpc.get_slot().await?;

    // ProgramData { slot, upgrade_authority_address: Some(authority) } ++ elf
    let mut pd = Vec::with_capacity(45 + so.len());
    pd.extend_from_slice(&3u32.to_le_bytes());
    pd.extend_from_slice(&slot.to_le_bytes());
    pd.push(1);
    pd.extend_from_slice(authority.as_ref());
    assert_eq!(pd.len(), 45, "UpgradeableLoaderState::size_of_programdata_metadata()");
    pd.extend_from_slice(so);

    // Program { programdata_address }
    let mut p = Vec::with_capacity(36);
    p.extend_from_slice(&2u32.to_le_bytes());
    p.extend_from_slice(programdata.as_ref());
    assert_eq!(p.len(), 36, "UpgradeableLoaderState::size_of_program()");

    let rent_pd = rpc.get_minimum_balance_for_rent_exemption(pd.len()).await?;
    let rent_p = rpc.get_minimum_balance_for_rent_exemption(p.len()).await?;
    set_account(rpc, &programdata, rent_pd, &pd, &loader, false).await?;
    set_account(rpc, program_id, rent_p, &p, &loader, true).await?;

    let acct = rpc.get_account(program_id).await?;
    anyhow::ensure!(acct.executable, "{program_id} not executable after deploy");
    Ok(())
}

async fn time_travel_to_slot(rpc: &RpcClient, slot: u64) -> Result<()> {
    rpc.send::<serde_json::Value>(
        RpcRequest::Custom { method: "surfnet_timeTravel" },
        json!([{ "absoluteSlot": slot }]),
    )
    .await
    .context("surfnet_timeTravel")?;
    Ok(())
}

struct LocalClock {
    slot: u64,
    unix_timestamp: i64,
}

async fn clock(rpc: &RpcClient) -> Result<LocalClock> {
    let d = rpc.get_account(&pk(CLOCK_SYSVAR)).await?.data;
    anyhow::ensure!(d.len() >= 40, "clock sysvar is {} bytes", d.len());
    Ok(LocalClock {
        slot: u64::from_le_bytes(d[0..8].try_into().unwrap()),
        unix_timestamp: i64::from_le_bytes(d[32..40].try_into().unwrap()),
    })
}

// ---------------------------------------------------------------------------

async fn send(rpc: &RpcClient, payer: &Keypair, ix: Instruction, label: &str) -> Result<()> {
    let bh = rpc.get_latest_blockhash().await.context("blockhash")?;
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[payer], msg, bh);
    match rpc.send_and_confirm_transaction(&tx).await {
        Ok(sig) => {
            println!("  {label:<22} ok   {sig}");
            Ok(())
        }
        Err(e) => {
            println!("  {label:<22} FAILED\n      {e}");
            Err(anyhow!("{label} failed"))
        }
    }
}

/// The 320-byte quote account, read by offset exactly as a consumer would.
/// Offsets are asserted in `svi-core/tests/core.rs::layout_is_frozen`.
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

impl Quote {
    fn decode(data: &[u8]) -> Result<Self> {
        let d = data.get(8..).ok_or_else(|| anyhow!("quote account too small"))?;
        anyhow::ensure!(d.len() >= 304, "quote payload is {} bytes", d.len());
        let at = |o: usize| u64::from_le_bytes(d[o..o + 8].try_into().unwrap());
        Ok(Quote {
            base_amount: at(224),
            quote_amount: at(232),
            lower: at(240),
            upper: at(248),
            observed_slot: at(256),
            valid_until_slot: at(280),
            sequence: at(288),
            status_flags: at(296),
        })
    }
}

fn read_so(rel: &str) -> Result<Vec<u8>> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), rel].iter().collect();
    std::fs::read(&path).with_context(|| {
        format!("{} is missing -- run `anchor build` in that workspace first", path.display())
    })
}

#[tokio::test]
async fn publishes_a_real_xsol_nav_quote_on_a_surfnet() -> Result<()> {
    let url = env::var("SVI_SURFNET_RPC").unwrap_or_else(|_| "http://127.0.0.1:8899".into());
    let mainnet = env::var("SVI_MAINNET_RPC")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into());
    let kp_path = env::var("SVI_KEYPAIR").unwrap_or_else(|_| {
        format!("{}/.config/solana/id.json", env::var("HOME").unwrap_or_default())
    });

    let rpc = RpcClient::new_with_commitment(url.clone(), CommitmentConfig::confirmed());
    if rpc.get_version().await.is_err() {
        eprintln!("\nSKIPPED: no Surfnet at {}. Start one with `surfpool start`.\n", redact(&url));
        return Ok(());
    }
    println!("\nsurfnet   {}\nmainnet   {}", redact(&url), redact(&mainnet));

    let svi_core = pk(SVI_CORE_ID);
    let adapter = pk(ADAPTER_ID);
    let payer = read_keypair_file(&kp_path).map_err(|e| anyhow!("keypair {kp_path}: {e}"))?;
    if rpc.get_balance(&payer.pubkey()).await? < 1_000_000_000 {
        let sig = rpc.request_airdrop(&payer.pubkey(), 10_000_000_000).await?;
        rpc.confirm_transaction(&sig).await?;
    }

    // ---- 1. deploy the binaries just built ---------------------------------
    let core_so = read_so("../../../svi-core/target/deploy/svi_core.so")?;
    let adapter_so = read_so("../../target/deploy/svi_hylo_adapter.so")?;
    deploy(&rpc, &svi_core, &core_so, &payer.pubkey()).await?;
    deploy(&rpc, &adapter, &adapter_so, &payer.pubkey()).await?;
    println!("  deployed               svi-core {} bytes, adapter {} bytes", core_so.len(), adapter_so.len());

    // ---- 2. fresh snapshot of Hylo's accounts ------------------------------
    let hylo_accounts = hylo_quotes::protocol_state::ProtocolAccounts::lst_pubkeys();
    let (hylo_state, xsol_mint, sol_usd) = (hylo_accounts[0], hylo_accounts[1], hylo_accounts[2]);
    let main = RpcClient::new_with_commitment(mainnet, CommitmentConfig::confirmed());
    match main.get_multiple_accounts(&[hylo_state, xsol_mint, sol_usd]).await {
        Ok(accts) => {
            for (key, a) in [hylo_state, xsol_mint, sol_usd].iter().zip(accts) {
                let a = a.ok_or_else(|| anyhow!("{key} missing on mainnet"))?;
                set_account(&rpc, key, a.lamports, &a.data, &a.owner, a.executable).await?;
            }
            println!("  snapshot               3 Hylo accounts re-cloned from mainnet");
        }
        Err(e) => println!(
            "  snapshot               SKIPPED (mainnet unreachable: {e}); using what the Surfnet has"
        ),
    }

    // ---- 3. align the clock with the snapshot ------------------------------
    let hylo = {
        let d = rpc.get_account(&hylo_state).await?.data;
        Hylo::try_deserialize(&mut d.as_ref())?
    };
    let pyth = {
        let d = rpc.get_account(&sol_usd).await?.data;
        PriceUpdateV2::try_deserialize(&mut d.as_ref())?
    };
    let mut c = clock(&rpc).await?;
    let interval = hylo.oracle_interval_secs;
    println!(
        "  clock                  slot {} ts {} | pyth posted_slot {} publish_time {} | hylo interval {}s | cache epoch {} vs clock epoch {}",
        c.slot, c.unix_timestamp, pyth.posted_slot, pyth.price_message.publish_time, interval,
        hylo.total_sol_cache.current_update_epoch, c.slot / SLOTS_PER_EPOCH
    );
    if c.slot < pyth.posted_slot {
        time_travel_to_slot(&rpc, pyth.posted_slot + 2).await?;
        c = clock(&rpc).await?;
        println!("  clock                  moved forward to slot {}", c.slot);
    }
    // hylo-core: posted_slot <= slot <= posted_slot + interval * slots/sec
    let slot_interval = interval.saturating_mul(5).saturating_div(2);
    if c.slot > pyth.posted_slot + slot_interval {
        bail!(
            "this Surfnet's slot ({}) is {} ahead of the snapshot's ({}) and the clock only moves \
             forward. Restart it: surfpool start",
            c.slot, c.slot - pyth.posted_slot, pyth.posted_slot
        );
    }

    // ---- 4. PDAs ------------------------------------------------------------
    let fid = feed_id();
    let (descriptor, _) = Pubkey::find_program_address(&[b"descriptor", &fid], &svi_core);
    let (quote, _) = Pubkey::find_program_address(&[b"quote", &fid], &svi_core);
    let (config, _) = Pubkey::find_program_address(&[b"config"], &adapter);
    let (adapter_authority, _) = Pubkey::find_program_address(&[b"authority"], &adapter);

    // ---- 5. initialize_feed (idempotent) -----------------------------------
    if rpc.get_account(&descriptor).await.is_err() {
        let mut data = IX_INITIALIZE_FEED.to_vec();
        data.extend_from_slice(&fid);
        data.extend_from_slice(adapter.as_ref());
        data.extend_from_slice(adapter_authority.as_ref());
        data.extend_from_slice(config.as_ref());
        data.extend_from_slice(xsol_mint.as_ref());
        data.extend_from_slice(Pubkey::default().as_ref()); // quote_mint: fiat
        data.extend_from_slice(&[0u8; 32]); // methodology_hash, unfrozen
        data.extend_from_slice(&MAX_AGE_SLOTS.to_le_bytes());
        data.extend_from_slice(&USD.to_le_bytes());
        data.extend_from_slice(&[VALUE_TYPE_PROTOCOL_NAV, XSOL_DECIMALS, QUOTE_DECIMALS]);
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
        println!("  initialize_feed        skipped (descriptor exists)");
    }

    // ---- 6. adapter initialize (idempotent) --------------------------------
    if rpc.get_account(&config).await.is_err() {
        let mut data = IX_ADAPTER_INIT.to_vec();
        data.extend_from_slice(svi_core.as_ref());
        data.extend_from_slice(descriptor.as_ref());
        data.extend_from_slice(quote.as_ref());
        data.extend_from_slice(&[0u8; 32]);
        data.extend_from_slice(&SDK_REVISION);
        data.extend_from_slice(&MAX_BAND_BPS.to_le_bytes());
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
        println!("  adapter initialize     skipped (config exists)");
    }

    // ---- 7. the actual thing -----------------------------------------------
    let before = rpc.get_account(&quote).await.ok().and_then(|a| Quote::decode(&a.data).ok());
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

    // ---- 8. read back and assert -------------------------------------------
    let q = Quote::decode(&rpc.get_account(&quote).await?.data)?;
    let usd = |bits: u64| format!("{}.{:09}", bits / 1_000_000_000, bits % 1_000_000_000);
    println!("\n  PUBLISHED QUOTE  {quote}");
    println!("    quote_amount   $ {}", usd(q.quote_amount));
    println!("    lower / upper  $ {} .. $ {}", usd(q.lower), usd(q.upper));
    println!("    base_amount      {} (1 whole xSOL)", q.base_amount);
    println!("    observed_slot    {}   valid_until {}", q.observed_slot, q.valid_until_slot);
    println!("    sequence         {}   flags {:#b}", q.sequence, q.status_flags);

    assert_eq!(q.base_amount, 1_000_000, "one whole xSOL at 6 decimals");
    assert!(q.quote_amount > 0, "a NAV of zero is a refusal that slipped through");
    assert!(q.lower <= q.quote_amount && q.quote_amount <= q.upper, "bounds incoherent");
    assert!(q.upper - q.lower <= q.quote_amount * MAX_BAND_BPS / 10_000, "band wider than configured");
    assert_eq!(q.valid_until_slot, q.observed_slot + MAX_AGE_SLOTS);
    if let Some(b) = before {
        assert_eq!(q.sequence, b.sequence + 1, "sequence must advance by one per publish");
    }

    // ---- 9. independent off-chain recomputation over the same frozen state --
    let provider = hylo_quotes::protocol_state::RpcStateProvider::new(Arc::new(
        RpcClient::new_with_commitment(url, CommitmentConfig::confirmed()),
    ));
    let ctx = provider.fetch_lst_context().await.context("off-chain hylo-core load")?;
    let redeem = ctx.levercoin_redeem_nav().context("off-chain redeem nav")?;
    println!("\n  off-chain hylo-core:   $ {redeem}");
    assert_eq!(
        q.quote_amount, redeem.bits,
        "on-chain and off-chain hylo-core disagree over identical state"
    );
    println!("  MATCH: on-chain program and off-chain reader agree to the last digit.\n");
    Ok(())
}

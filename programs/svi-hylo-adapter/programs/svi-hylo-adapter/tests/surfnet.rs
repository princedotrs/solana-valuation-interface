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
//! * Pins the Surfnet clock to the snapshot's own slot and time for the one
//!   instruction that reads it. Hylo's oracle window is 10 seconds and
//!   hylo-core needs both `posted_slot <= slot <= posted_slot + 25` and
//!   `unix_timestamp <= publish_time + 10`. A Surfnet's slot counter drifts
//!   from mainnet's (fixed 400 ms ticks vs. mainnet's real cadence) while its
//!   timestamp tracks wall time, so on any Surfnet older than a few minutes
//!   the two cannot both hold, and `surfnet_timeTravel` only moves forward,
//!   dragging the timestamp along. Pausing the clock and writing the Clock
//!   sysvar directly (`surfnet_setAccount`; LiteSVM parses it and rebuilds
//!   its sysvar cache) evaluates the refresh exactly as a fork taken at
//!   `posted_slot` would. No Hylo or Pyth byte is altered.
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

use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token::Mint;
use hylo_core::exchange_context::{ExchangeContext, LstExchangeContext};
use hylo_core::fees::controller::LevercoinFees;
use hylo_core::lst::total_sol_cache::TotalSolCache;
use hylo_core::pyth::OracleConfig;
use hylo_idl::exchange::accounts::Hylo;
use hylo_idl::pda;
use hylo_idl::tokens::{TokenMint, XSOL};
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

async fn cheatcode(rpc: &RpcClient, method: &'static str, params: serde_json::Value) -> Result<()> {
    rpc.send::<serde_json::Value>(RpcRequest::Custom { method }, params)
        .await
        .with_context(|| method.to_string())?;
    Ok(())
}

/// Write the Clock sysvar. Only meaningful while the clock is paused; the
/// next produced block rewrites it from surfpool's internal state.
async fn pin_clock(rpc: &RpcClient, c: &Clock) -> Result<()> {
    let key = pk(CLOCK_SYSVAR);
    let existing = rpc.get_account(&key).await?;
    let mut d = Vec::with_capacity(40);
    d.extend_from_slice(&c.slot.to_le_bytes());
    d.extend_from_slice(&c.epoch_start_timestamp.to_le_bytes());
    d.extend_from_slice(&c.epoch.to_le_bytes());
    d.extend_from_slice(&c.leader_schedule_epoch.to_le_bytes());
    d.extend_from_slice(&c.unix_timestamp.to_le_bytes());
    set_account(rpc, &key, existing.lamports, &d, &existing.owner, false).await?;
    let back = clock(rpc).await?;
    anyhow::ensure!(
        back.slot == c.slot && back.unix_timestamp == c.unix_timestamp,
        "clock did not take: wrote slot {} ts {}, read slot {} ts {}",
        c.slot, c.unix_timestamp, back.slot, back.unix_timestamp
    );
    Ok(())
}

/// The clock sysvar is five little-endian 8-byte fields in declaration order.
async fn clock(rpc: &RpcClient) -> Result<Clock> {
    let d = rpc.get_account(&pk(CLOCK_SYSVAR)).await?.data;
    anyhow::ensure!(d.len() >= 40, "clock sysvar is {} bytes", d.len());
    let u = |o: usize| u64::from_le_bytes(d[o..o + 8].try_into().unwrap());
    let i = |o: usize| i64::from_le_bytes(d[o..o + 8].try_into().unwrap());
    Ok(Clock {
        slot: u(0),
        epoch_start_timestamp: i(8),
        epoch: u(16),
        leader_schedule_epoch: u(24),
        unix_timestamp: i(32),
    })
}

/// hylo-quotes' `build_lst_exchange_context`, reproduced with hylo-core's own
/// `offchain` conversions rather than the adapter's `idl_bridge`, so this is
/// an independent reading of the same bytes and not the program checking
/// itself. (hylo-quotes itself cannot be a dev-dependency: see Cargo.toml.)
async fn offchain_redeem_nav(rpc: &RpcClient, clock: Clock, hylo_state: &Pubkey, xsol_mint: &Pubkey, sol_usd: &Pubkey) -> Result<u64> {
    let accts = rpc.get_multiple_accounts(&[*hylo_state, *xsol_mint, *sol_usd]).await?;
    let [Some(h), Some(m), Some(p)] = accts.as_slice() else { bail!("Hylo account missing") };
    let hylo = Hylo::try_deserialize(&mut h.data.as_slice())?;
    let mint = Mint::try_deserialize(&mut m.data.as_slice())?;
    let pyth = PriceUpdateV2::try_deserialize(&mut p.data.as_slice())?;
    let total_sol_cache: TotalSolCache = hylo.total_sol_cache.into();
    let fees: LevercoinFees = hylo.levercoin_fees.into();
    let ctx = LstExchangeContext::load(
        clock,
        &total_sol_cache,
        hylo.stablecoin_mint_threshold.try_into()?,
        OracleConfig::new(hylo.oracle_interval_secs, hylo.oracle_conf_tolerance.try_into()?),
        fees,
        &pyth,
        hylo.virtual_stablecoin.into(),
        Some(&mint),
        hylo.lst_sell_curve_config.into(),
        hylo.lst_buy_curve_config.into(),
    )
    .context("off-chain LstExchangeContext::load")?;
    Ok(ctx.levercoin_redeem_nav().context("off-chain redeem nav")?.bits)
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

/// A `Quote` account is 8 discriminator bytes plus a 320-byte payload, and the
/// layout is frozen (`svi-core/tests/core.rs::layout_is_frozen`). Anything
/// shorter is not a partially-written quote we could read the front of — it is
/// a different account, and decoding it would produce numbers with no meaning.
const QUOTE_PAYLOAD_LEN: usize = 320;

impl Quote {
    fn decode(data: &[u8]) -> Result<Self> {
        let d = data.get(8..).ok_or_else(|| anyhow!("quote account too small"))?;
        anyhow::ensure!(
            d.len() >= QUOTE_PAYLOAD_LEN,
            "quote payload is {} bytes, expected {QUOTE_PAYLOAD_LEN}",
            d.len()
        );
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
    // The same three the program requires: state PDA, xSOL mint, and the Pyth
    // feed Hylo names in its own state (asserted below once state is loaded).
    let (hylo_state, xsol_mint, sol_usd) =
        (pda::HYLO, XSOL::MINT, hylo_core::pyth::SOL_USD.address);
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

    // ---- 3. the clock the refresh will see --------------------------------
    let hylo = {
        let d = rpc.get_account(&hylo_state).await?.data;
        Hylo::try_deserialize(&mut d.as_ref())?
    };
    let pyth = {
        let d = rpc.get_account(&sol_usd).await?.data;
        PriceUpdateV2::try_deserialize(&mut d.as_ref())?
    };
    anyhow::ensure!(
        hylo.sol_usd_oracle == sol_usd,
        "Hylo state names oracle {} but SDK says {sol_usd}", hylo.sol_usd_oracle
    );
    let live = clock(&rpc).await?;
    let pinned = Clock {
        slot: pyth.posted_slot + 1,
        epoch: (pyth.posted_slot + 1) / SLOTS_PER_EPOCH,
        unix_timestamp: pyth.price_message.publish_time + 1,
        epoch_start_timestamp: 0,
        leader_schedule_epoch: 0,
    };
    println!(
        "  clock (live)           slot {} ts {} | pyth posted_slot {} publish_time {} | hylo interval {}s",
        live.slot, live.unix_timestamp, pyth.posted_slot, pyth.price_message.publish_time,
        hylo.oracle_interval_secs
    );
    println!(
        "  clock (pinned)         slot {} ts {} epoch {} | hylo cache epoch {}",
        pinned.slot, pinned.unix_timestamp, pinned.epoch, hylo.total_sol_cache.current_update_epoch
    );

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

    // ---- 7. the actual thing, under the pinned clock ------------------------
    let before = rpc.get_account(&quote).await.ok().and_then(|a| Quote::decode(&a.data).ok());
    let refresh = Instruction {
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
    };
    // Resume first in case an earlier run died while paused.
    cheatcode(&rpc, "surfnet_resumeClock", json!([])).await?;
    cheatcode(&rpc, "surfnet_pauseClock", json!([])).await?;
    // The pause is relayed to the clock thread asynchronously; let any tick
    // already in flight land before pinning, or it would overwrite the pin.
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    let outcome: Result<()> = async {
        pin_clock(&rpc, &pinned).await?;
        let bh = rpc.get_latest_blockhash().await?;
        let tx = Transaction::new(&[&payer], Message::new(&[refresh], Some(&payer.pubkey())), bh);
        // Preflight runs against the pinned clock too, so a refusal surfaces
        // here with the program's logs. Execution happens on receipt; only the
        // "confirmed" status waits for a block, which needs the clock back.
        let sig = match rpc.send_transaction(&tx).await {
            Ok(sig) => sig,
            Err(e) => {
                println!("  refresh_xsol_nav       FAILED\n      {e}");
                bail!("refresh_xsol_nav failed");
            }
        };
        let want = before.as_ref().map(|b| b.sequence + 1).unwrap_or(1);
        for _ in 0..50 {
            if let Ok(a) = rpc.get_account(&quote).await {
                if Quote::decode(&a.data).map(|q| q.sequence >= want).unwrap_or(false) {
                    println!("  refresh_xsol_nav       ok   {sig}");
                    return Ok(());
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        bail!("refresh_xsol_nav {sig} was accepted but the quote never advanced to sequence {want}")
    }
    .await;
    cheatcode(&rpc, "surfnet_resumeClock", json!([])).await?;
    outcome?;

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
    let redeem = offchain_redeem_nav(&rpc, pinned, &hylo_state, &xsol_mint, &sol_usd).await?;
    println!("\n  off-chain hylo-core:   $ {}", usd(redeem));
    assert_eq!(
        q.quote_amount, redeem,
        "on-chain and off-chain hylo-core disagree over identical state"
    );
    println!("  MATCH: on-chain program and off-chain reader agree to the last digit.\n");
    Ok(())
}

//! svi-keeper: crank the Hylo xSOL NAV feed, and watch what it publishes.
//!
//!     svi-keeper crank [--interval 5] [--surfnet]          # terminal 1 (keeper)
//!     svi-keeper watch                                     # terminal 2 (our quote)
//!     svi-keeper watch --pyth                              # terminal 3 (the input)
//!
//! The keeper is permissionless by design: anyone can run it, it holds no
//! privileged key, and svi-core only accepts the value because the adapter's
//! own PDA signed the CPI. A refusal (stale oracle, cache epoch, band too
//! wide) is logged and the loop continues; the previous quote simply ages
//! past its `valid_until_slot`. *Fail stale, never fail wrong.*
//!
//! `--surfnet` is for a local fork, which freezes Pyth at fork time while its
//! clock keeps running. Each tick then re-clones Hylo's three accounts from
//! mainnet and pins the Surfnet clock to that snapshot's own slot and time for
//! the one instruction that reads it, exactly as tests/surfnet.rs does. On
//! mainnet none of that applies and the keeper just sends the instruction.
//!
//! Environment (flags override):
//!     SVI_RPC           default http://127.0.0.1:8899
//!     SVI_MAINNET_RPC   default https://api.mainnet-beta.solana.com  (--surfnet only)
//!     SVI_KEYPAIR       default ~/.config/solana/id.json

use std::env;
use std::time::Duration;

pub mod stock;

use anchor_lang::solana_program::{
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};
use anchor_lang::AccountDeserialize;
use anyhow::{anyhow, bail, Context, Result};
use hylo_idl::pda;
use hylo_idl::tokens::{TokenMint, XSOL};
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;
use serde_json::json;
use solana_commitment_config::CommitmentConfig;
use solana_keypair::{read_keypair_file, Keypair};
use solana_message::Message;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::request::RpcRequest;
use solana_sha256_hasher::hashv;
use solana_signer::Signer;
use solana_transaction::Transaction;

const SVI_CORE_ID: &str = "H6495aW1Cxyoz2R7MuYVHF3Pu3FumFE9cZwKSJCR8oHH";
const ADAPTER_ID: &str = "FfK5xyE3v3GT2F9wzqhpLMHx7zCJsGPQrHAvxtgL3Mpr";
const CLOCK_SYSVAR: &str = "SysvarC1ock11111111111111111111111111111111";
const SLOTS_PER_EPOCH: u64 = 432_000;

const IX_INITIALIZE_FEED: [u8; 8] = [167, 251, 140, 58, 66, 138, 187, 95];
const IX_ADAPTER_INIT: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
const IX_REFRESH: [u8; 8] = [133, 61, 124, 93, 60, 229, 233, 255];

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

/// Never print an RPC URL: it may carry an API key.
fn redact(url: &str) -> String {
    match url.find('?') {
        Some(i) => format!("{}?<redacted>", &url[..i]),
        None => url.to_string(),
    }
}

fn arg(flag: &str) -> Option<String> {
    let a: Vec<String> = env::args().collect();
    a.iter().position(|x| x == flag).and_then(|i| a.get(i + 1).cloned())
}

fn has(flag: &str) -> bool {
    env::args().any(|x| x == flag)
}

fn usd(bits: u64) -> String {
    format!("{}.{:09}", bits / 1_000_000_000, bits % 1_000_000_000)
}

// ---------------------------------------------------------------------------
// Feed addresses
// ---------------------------------------------------------------------------

struct Feed {
    svi_core: Pubkey,
    adapter: Pubkey,
    descriptor: Pubkey,
    quote: Pubkey,
    config: Pubkey,
    adapter_authority: Pubkey,
    hylo_state: Pubkey,
    xsol_mint: Pubkey,
    sol_usd: Pubkey,
}

impl Feed {
    fn derive() -> Self {
        let svi_core = pk(SVI_CORE_ID);
        let adapter = pk(ADAPTER_ID);
        let fid = feed_id();
        Feed {
            svi_core,
            adapter,
            descriptor: Pubkey::find_program_address(&[b"descriptor", &fid], &svi_core).0,
            quote: Pubkey::find_program_address(&[b"quote", &fid], &svi_core).0,
            config: Pubkey::find_program_address(&[b"config"], &adapter).0,
            adapter_authority: Pubkey::find_program_address(&[b"authority"], &adapter).0,
            hylo_state: pda::HYLO,
            xsol_mint: XSOL::MINT,
            sol_usd: hylo_core::pyth::SOL_USD.address,
        }
    }

    fn refresh_ix(&self, payer: &Pubkey) -> Instruction {
        Instruction {
            program_id: self.adapter,
            accounts: vec![
                AccountMeta::new(*payer, true),
                AccountMeta::new_readonly(self.config, false),
                AccountMeta::new_readonly(self.hylo_state, false),
                AccountMeta::new_readonly(self.xsol_mint, false),
                AccountMeta::new_readonly(self.sol_usd, false),
                AccountMeta::new_readonly(self.descriptor, false),
                AccountMeta::new(self.quote, false),
                AccountMeta::new_readonly(self.adapter_authority, false),
                AccountMeta::new_readonly(self.svi_core, false),
            ],
            data: IX_REFRESH.to_vec(),
        }
    }
}

// ---------------------------------------------------------------------------
// Quote account
// ---------------------------------------------------------------------------

/// The 320-byte quote, read by offset as any consumer would. Offsets are
/// asserted in `svi-core/tests/core.rs::layout_is_frozen`.
#[derive(Clone, Copy)]
struct Quote {
    base_amount: u64,
    quote_amount: u64,
    lower: u64,
    upper: u64,
    observed_slot: u64,
    observed_unix_ts: i64,
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
        let u = |o: usize| u64::from_le_bytes(d[o..o + 8].try_into().unwrap());
        Ok(Quote {
            base_amount: u(224),
            quote_amount: u(232),
            lower: u(240),
            upper: u(248),
            observed_slot: u(256),
            observed_unix_ts: u(264) as i64,
            valid_until_slot: u(280),
            sequence: u(288),
            status_flags: u(296),
        })
    }

    fn flag_names(&self) -> String {
        const NAMES: [&str; 6] = [
            "ZERO_SUPPLY_DEFAULT",
            "DESTABILIZED",
            "OPERATIONS_HALTED",
            "SELL_ZONE",
            "BUY_ZONE",
            "EPOCH_BOUNDARY_CPI",
        ];
        let set: Vec<&str> = NAMES
            .iter()
            .enumerate()
            .filter(|(i, _)| self.status_flags & (1 << i) != 0)
            .map(|(_, n)| *n)
            .collect();
        if set.is_empty() { "none".into() } else { set.join(" | ") }
    }
}

// ---------------------------------------------------------------------------
// Surfnet cheatcodes (only used with --surfnet)
// ---------------------------------------------------------------------------

async fn cheatcode(rpc: &RpcClient, method: &'static str, params: serde_json::Value) -> Result<()> {
    rpc.send::<serde_json::Value>(RpcRequest::Custom { method }, params)
        .await
        .with_context(|| method.to_string())?;
    Ok(())
}

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
        json!([key.to_string(), {
            "lamports": lamports, "data": hex::encode(data), "owner": owner.to_string(),
            "executable": executable, "rent_epoch": 0u64,
        }]),
    )
    .await
    .with_context(|| format!("surfnet_setAccount {key}"))?;
    Ok(())
}

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
    anyhow::ensure!(back.slot == c.slot, "clock pin did not take");
    Ok(())
}

/// Re-clone Hylo's three accounts from mainnet and return the clock a fork
/// taken at that snapshot would have.
async fn snapshot(rpc: &RpcClient, mainnet: &RpcClient, f: &Feed) -> Result<Clock> {
    let keys = [f.hylo_state, f.xsol_mint, f.sol_usd];
    let accts = mainnet.get_multiple_accounts(&keys).await.context("mainnet fetch")?;
    for (key, a) in keys.iter().zip(accts) {
        let a = a.ok_or_else(|| anyhow!("{key} missing on mainnet"))?;
        set_account(rpc, key, a.lamports, &a.data, &a.owner, a.executable).await?;
    }
    let pyth = PriceUpdateV2::try_deserialize(&mut rpc.get_account(&f.sol_usd).await?.data.as_ref())?;
    Ok(Clock {
        slot: pyth.posted_slot + 1,
        epoch: (pyth.posted_slot + 1) / SLOTS_PER_EPOCH,
        unix_timestamp: pyth.price_message.publish_time + 1,
        epoch_start_timestamp: 0,
        leader_schedule_epoch: 0,
    })
}

// ---------------------------------------------------------------------------
// crank
// ---------------------------------------------------------------------------

/// Anchor error codes this adapter and svi-core can return, for the log line.
fn explain(err: &str) -> String {
    let code = err
        .find("custom program error: 0x")
        .and_then(|i| err[i + 24..].split(|c: char| !c.is_ascii_hexdigit()).next())
        .and_then(|h| u32::from_str_radix(h, 16).ok());
    match code {
        Some(6005) => "ContextUnavailable (6005): hylo-core refused for a reason it does not distinguish".into(),
        Some(6008) => "BandTooWide (6008): uncertainty wider than max_band_bps".into(),
        Some(6011) => "HyloCacheStale (6011): TotalSolCache epoch != clock epoch; Hylo needs update_lst_prices".into(),
        Some(6012) => "OracleStale (6012): Pyth SOL/USD older than Hylo's interval".into(),
        Some(6013) => "OracleConfidenceTooWide (6013): Pyth confidence outside Hylo's tolerance".into(),
        Some(c) => format!("program error {c}"),
        None => err.lines().next().unwrap_or(err).to_string(),
    }
}

async fn ensure_initialized(rpc: &RpcClient, payer: &Keypair, f: &Feed) -> Result<()> {
    let fid = feed_id();
    if rpc.get_account(&f.descriptor).await.is_err() {
        let mut data = IX_INITIALIZE_FEED.to_vec();
        data.extend_from_slice(&fid);
        data.extend_from_slice(f.adapter.as_ref());
        data.extend_from_slice(f.adapter_authority.as_ref());
        data.extend_from_slice(f.config.as_ref());
        data.extend_from_slice(f.xsol_mint.as_ref());
        data.extend_from_slice(Pubkey::default().as_ref());
        data.extend_from_slice(&[0u8; 32]);
        data.extend_from_slice(&MAX_AGE_SLOTS.to_le_bytes());
        data.extend_from_slice(&840u16.to_le_bytes());
        data.extend_from_slice(&[1, 6, 9]);
        send(rpc, payer, Instruction {
            program_id: f.svi_core,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(f.descriptor, false),
                AccountMeta::new(f.quote, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data,
        }).await.context("initialize_feed")?;
        println!("initialized feed        {}", f.descriptor);
    }
    if rpc.get_account(&f.config).await.is_err() {
        let mut data = IX_ADAPTER_INIT.to_vec();
        data.extend_from_slice(f.svi_core.as_ref());
        data.extend_from_slice(f.descriptor.as_ref());
        data.extend_from_slice(f.quote.as_ref());
        data.extend_from_slice(&[0u8; 32]);
        data.extend_from_slice(&SDK_REVISION);
        data.extend_from_slice(&MAX_BAND_BPS.to_le_bytes());
        send(rpc, payer, Instruction {
            program_id: f.adapter,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(f.config, false),
                AccountMeta::new_readonly(f.adapter_authority, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data,
        }).await.context("adapter initialize")?;
        println!("initialized adapter     {}", f.config);
    }
    Ok(())
}

async fn send(rpc: &RpcClient, payer: &Keypair, ix: Instruction) -> Result<String> {
    let bh = rpc.get_latest_blockhash().await?;
    let tx = Transaction::new(&[payer], Message::new(&[ix], Some(&payer.pubkey())), bh);
    Ok(rpc.send_and_confirm_transaction(&tx).await?.to_string())
}

async fn crank(rpc_url: String, kp_path: String) -> Result<()> {
    let interval = arg("--interval").and_then(|s| s.parse().ok()).unwrap_or(5u64);
    let surfnet = has("--surfnet");
    let rpc = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());
    let payer = read_keypair_file(&kp_path).map_err(|e| anyhow!("keypair {kp_path}: {e}"))?;
    let f = Feed::derive();
    let mainnet = surfnet.then(|| {
        let url = env::var("SVI_MAINNET_RPC")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into());
        RpcClient::new_with_commitment(url, CommitmentConfig::confirmed())
    });

    println!("svi-keeper crank        every {interval}s{}", if surfnet { "  (surfnet mode)" } else { "" });
    println!("rpc                     {}", redact(&rpc_url));
    println!("keeper                  {}  (any key works; the adapter PDA is the writer)", payer.pubkey());
    println!("quote account           {}\n", f.quote);

    for (name, key) in [("svi-core", f.svi_core), ("adapter", f.adapter)] {
        let ok = rpc.get_account(&key).await.map(|a| a.executable).unwrap_or(false);
        anyhow::ensure!(ok, "{name} ({key}) is not deployed on this cluster");
    }
    ensure_initialized(&rpc, &payer, &f).await?;

    loop {
        let started = std::time::Instant::now();
        let outcome: Result<String> = async {
            if let Some(mainnet) = &mainnet {
                let pinned = snapshot(&rpc, mainnet, &f).await?;
                cheatcode(&rpc, "surfnet_resumeClock", json!([])).await?;
                cheatcode(&rpc, "surfnet_pauseClock", json!([])).await?;
                tokio::time::sleep(Duration::from_millis(600)).await;
                let r: Result<String> = async {
                    pin_clock(&rpc, &pinned).await?;
                    let bh = rpc.get_latest_blockhash().await?;
                    let tx = Transaction::new(
                        &[&payer],
                        Message::new(&[f.refresh_ix(&payer.pubkey())], Some(&payer.pubkey())),
                        bh,
                    );
                    let sig = rpc.send_transaction(&tx).await.map_err(|e| anyhow!("{e}"))?;
                    Ok(sig.to_string())
                }
                .await;
                cheatcode(&rpc, "surfnet_resumeClock", json!([])).await?;
                r
            } else {
                send(&rpc, &payer, f.refresh_ix(&payer.pubkey())).await
            }
        }
        .await;

        let now = chrono_like_now();
        match outcome {
            Ok(sig) => {
                let q = Quote::decode(&rpc.get_account(&f.quote).await?.data)?;
                println!(
                    "{now}  published  $ {}  [{} .. {}]  seq {}  slot {}  {}  {}",
                    usd(q.quote_amount), usd(q.lower), usd(q.upper), q.sequence, q.observed_slot,
                    q.flag_names(), &sig[..16]
                );
            }
            Err(e) => println!("{now}  REFUSED    {}", explain(&e.to_string())),
        }
        let elapsed = started.elapsed();
        if elapsed < Duration::from_secs(interval) {
            tokio::time::sleep(Duration::from_secs(interval) - elapsed).await;
        }
    }
}

fn chrono_like_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{:02}:{:02}:{:02}Z", (secs / 3600) % 24, (secs / 60) % 60, secs % 60)
}

// ---------------------------------------------------------------------------
// watch
// ---------------------------------------------------------------------------

async fn watch(rpc_url: String) -> Result<()> {
    let show_pyth = has("--pyth");
    let once = has("--once");
    let rpc = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());
    let f = Feed::derive();
    loop {
        let slot = rpc.get_slot().await?;
        let mut out = String::new();
        if !once {
            out.push_str("\x1b[2J\x1b[H");
        }
        if show_pyth {
            let a = rpc.get_account(&f.sol_usd).await?;
            let p = PriceUpdateV2::try_deserialize(&mut a.data.as_ref())?;
            let scale = 10f64.powi(p.price_message.exponent);
            let price = p.price_message.price as f64 * scale;
            let conf = p.price_message.conf as f64 * scale;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            out.push_str(&format!(
                "PYTH SOL/USD  (the input)        {}\n\
                 ─────────────────────────────────────────────────────────\n\
                 price          $ {price:.4}  ± {conf:.4}\n\
                 publish_time   {}  ({}s ago)\n\
                 posted_slot    {}  ({} slots ago)\n\
                 \n\
                 Hylo will not price xSOL from this if it is older than its\n\
                 configured interval. SVI inherits that rule unchanged.\n",
                f.sol_usd,
                p.price_message.publish_time, now - p.price_message.publish_time,
                p.posted_slot, slot.saturating_sub(p.posted_slot),
            ));
        } else {
            match rpc.get_account(&f.quote).await {
                Ok(a) => {
                    let q = Quote::decode(&a.data)?;
                    let age = slot.saturating_sub(q.observed_slot);
                    let status = if slot <= q.valid_until_slot { "VALID" } else { "STALE — do not use" };
                    let band_bps = if q.quote_amount > 0 {
                        (q.upper - q.lower) * 10_000 / q.quote_amount
                    } else { 0 };
                    out.push_str(&format!(
                        "SVI  hylo-xsol-nav-v1  (derived on-chain)   {}\n\
                         ─────────────────────────────────────────────────────────\n\
                         NAV            $ {}  per xSOL\n\
                         bounds         $ {} .. $ {}   ({band_bps} bps)\n\
                         base           {} = 1 xSOL\n\
                         observed       slot {}  ts {}\n\
                         age            {age} slots (~{:.1}s)   valid until {}   {status}\n\
                         sequence       {}\n\
                         flags          {}\n\
                         \n\
                         No oracle publishes this number. It is Hylo's own math over\n\
                         Hylo's own state, computed by a program anyone can run, into\n\
                         an account anyone can read. Writer: adapter PDA only.\n",
                        f.quote,
                        usd(q.quote_amount), usd(q.lower), usd(q.upper), q.base_amount,
                        q.observed_slot, q.observed_unix_ts, age as f64 * 0.4,
                        q.valid_until_slot, q.sequence, q.flag_names(),
                    ));
                }
                Err(_) => out.push_str(&format!("quote account {} does not exist yet\n", f.quote)),
            }
        }
        out.push_str(&format!("\ncluster slot {slot}   rpc {}\n", redact(&rpc_url)));
        print!("{out}");
        if once {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// Tokenized stocks
//
// Separate functions rather than branches inside `crank`/`watch`: the Hylo
// path carries surfnet clock-pinning and a single quote, and threading a
// second shape through it would put the risk of breaking a working demo into
// every future edit here.
// ─────────────────────────────────────────────────────────────────────────────

fn deployments_path() -> std::path::PathBuf {
    arg("--deployments")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(stock::default_path)
}

async fn crank_stock(rpc_url: String, kp_path: String, symbol: String) -> Result<()> {
    let interval = arg("--interval").and_then(|s| s.parse().ok()).unwrap_or(10u64);
    let path = deployments_path();
    let deployments = stock::load(&path)?;
    let f = deployments.get(&symbol)?.clone();

    let rpc = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());
    let payer = read_keypair_file(&kp_path).map_err(|e| anyhow!("keypair {kp_path}: {e}"))?;

    println!("svi-keeper crank        {symbol} every {interval}s  ({})", deployments.cluster);
    println!("rpc                     {}", redact(&rpc_url));
    println!("keeper                  {}  (any key works; the adapter PDA is the writer)", payer.pubkey());
    println!("fair value quote        {}", f.fair_quote);
    println!("market price quote      {}\n", f.market_quote);

    for (name, key) in [("adapter", f.adapter), ("svi-core", f.svi_core)] {
        let ok = rpc.get_account(&key).await.map(|a| a.executable).unwrap_or(false);
        anyhow::ensure!(ok, "{name} ({key}) is not deployed on this cluster");
    }

    loop {
        let started = std::time::Instant::now();
        let now = chrono_like_now();
        match send(&rpc, &payer, f.refresh_ix(&payer.pubkey())).await {
            Ok(sig) => {
                let fair = read_quote(&rpc, &f.fair_quote).await;
                let market = read_quote(&rpc, &f.market_quote).await;
                match (fair, market) {
                    (Ok(fq), Ok(mq)) => println!(
                        "{now}  {symbol}  fair ${}  market ${}  {}  seq {}  {}",
                        usd(fq.quote_amount),
                        usd(mq.quote_amount),
                        premium(fq.quote_amount, mq.quote_amount),
                        fq.sequence,
                        stock::flag_names(fq.status_flags),
                    ),
                    _ => println!("{now}  published {}, but a quote did not read back", &sig[..16]),
                }
            }
            Err(e) => println!("{now}  REFUSED    {}", stock::explain_stock(&e.to_string())),
        }
        let elapsed = started.elapsed();
        if elapsed < Duration::from_secs(interval) {
            tokio::time::sleep(Duration::from_secs(interval) - elapsed).await;
        }
    }
}

async fn read_quote(rpc: &RpcClient, key: &Pubkey) -> Result<Quote> {
    Quote::decode(&rpc.get_account(key).await?.data)
}

/// Premium or discount of the token against its reference, as a signed string.
fn premium(fair: u64, market: u64) -> String {
    if fair == 0 {
        return "n/a".into();
    }
    let diff = market as i128 - fair as i128;
    let bps = diff.saturating_mul(10_000) / fair as i128;
    format!("{}{}bps", if bps > 0 { "+" } else { "" }, bps)
}

async fn watch_stock(rpc_url: String, symbol: String) -> Result<()> {
    let once = has("--once");
    let path = deployments_path();
    let deployments = stock::load(&path)?;
    let f = deployments.get(&symbol)?.clone();
    let rpc = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());

    loop {
        let slot = rpc.get_slot().await?;
        let mut out = String::new();
        if !once {
            out.push_str("\x1b[2J\x1b[H");
        }
        let fair = read_quote(&rpc, &f.fair_quote).await;
        let market = read_quote(&rpc, &f.market_quote).await;
        match (fair, market) {
            (Ok(fq), Ok(mq)) => {
                let age = slot.saturating_sub(fq.observed_slot);
                let status = if slot <= fq.valid_until_slot { "VALID" } else { "STALE - do not use" };
                out.push_str(&format!(
                    "SVI  {symbol}  ({})\n\
                     ---------------------------------------------------------\n\
                     fair value     $ {}   per token   <- what the share is worth\n\
                     market price   $ {}   per token   <- what the token trades at\n\
                     premium        {}\n\
                     \n\
                     fair bounds    $ {} .. $ {}\n\
                     market bounds  $ {} .. $ {}\n\
                     \n\
                     observed       slot {}   age {age} slots (~{:.0}s)   {status}\n\
                     sequence       {}\n\
                     flags          {}\n\
                     \n\
                     fair quote     {}\n\
                     market quote   {}\n",
                    deployments.cluster,
                    usd(fq.quote_amount), usd(mq.quote_amount),
                    premium(fq.quote_amount, mq.quote_amount),
                    usd(fq.lower), usd(fq.upper),
                    usd(mq.lower), usd(mq.upper),
                    fq.observed_slot, age as f64 * 0.4,
                    fq.sequence,
                    stock::flag_names(fq.status_flags),
                    f.fair_quote, f.market_quote,
                ));
            }
            _ => out.push_str(&format!(
                "{symbol}: quote accounts not readable yet ({} / {})\n",
                f.fair_quote, f.market_quote
            )),
        }
        out.push_str(&format!("\ncluster slot {slot}   rpc {}\n", redact(&rpc_url)));
        print!("{out}");
        if once {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let rpc_url = arg("--rpc")
        .or_else(|| env::var("SVI_RPC").ok())
        .unwrap_or_else(|| "http://127.0.0.1:8899".into());
    let kp_path = arg("--keypair").or_else(|| env::var("SVI_KEYPAIR").ok()).unwrap_or_else(|| {
        format!("{}/.config/solana/id.json", env::var("HOME").unwrap_or_default())
    });
    // `--feed` defaults to the original behaviour, so every existing
    // invocation of this tool keeps working unchanged.
    let selector = stock::FeedSelector::parse(
        &arg("--feed").unwrap_or_else(|| "hylo-xsol".into()),
    )?;

    match (env::args().nth(1).as_deref(), &selector) {
        (Some("crank"), stock::FeedSelector::HyloXsol) => crank(rpc_url, kp_path).await,
        (Some("watch"), stock::FeedSelector::HyloXsol) => watch(rpc_url).await,
        (Some("crank"), stock::FeedSelector::Stock(sym)) => {
            crank_stock(rpc_url, kp_path, sym.clone()).await
        }
        (Some("watch"), stock::FeedSelector::Stock(sym)) => {
            watch_stock(rpc_url, sym.clone()).await
        }
        _ => bail!(
            "usage:\n  \
             svi-keeper crank [--feed hylo-xsol|stock:SYM] [--interval SECS] [--surfnet] [--rpc URL] [--keypair PATH]\n  \
             svi-keeper watch [--feed hylo-xsol|stock:SYM] [--pyth] [--once] [--rpc URL]\n\n\
             stock feeds read their addresses from deployments.json (override with --deployments PATH)"
        ),
    }
}

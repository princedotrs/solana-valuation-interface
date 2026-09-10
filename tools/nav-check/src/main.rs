//! nav-check — reproduce Hylo's xSOL NAV from mainnet state, independently.
//!
//! This is the validation spike for SVI: it proves that the value SVI intends
//! to publish on-chain can be derived from public account data using Hylo's
//! own math library, with no cooperation from Hylo and no private arithmetic.
//!
//! It deliberately does NOT reimplement anything. Every number below comes out
//! of `hylo-core`, pinned to an exact git revision in Cargo.toml. That is the
//! whole point: SVI's number and Hylo's redemption math cannot disagree,
//! because they are the same code.
//!
//! Usage:
//!     cargo run -- [RPC_URL]
//!     RPC_URL=https://... cargo run
//!
//! Then compare `redeem NAV` against the xSOL price shown on hylo.so.

use std::env;
use std::sync::Arc;

use anyhow::{Context, Result};
use hylo_core::exchange_context::ExchangeContext;
use hylo_quotes::protocol_state::RpcStateProvider;
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;

#[tokio::main]
async fn main() -> Result<()> {
    let url = env::args()
        .nth(1)
        .or_else(|| env::var("RPC_URL").ok())
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());

    println!("nav-check — independent reproduction of Hylo xSOL NAV");
    println!("RPC: {url}\n");

    let rpc = Arc::new(RpcClient::new_with_commitment(
        url,
        CommitmentConfig::confirmed(),
    ));
    let slot = rpc.get_slot().await.context("get_slot")?;
    let epoch = rpc.get_epoch_info().await.context("get_epoch_info")?.epoch;

    // Four accounts, one call: Hylo protocol state, xSOL mint, Pyth SOL/USD,
    // and the Clock sysvar. All read at the same slot — which is exactly the
    // same-slot consistency the on-chain adapter gets for free inside one
    // transaction.
    let provider = RpcStateProvider::new(rpc);
    let ctx = provider
        .fetch_lst_context()
        .await
        .context("fetch_lst_context — if this fails with TotalSolCacheOutdated, \
                  a new epoch began and Hylo's LST crank has not run yet. That is \
                  the *correct* failure: spec §5.1, fail stale rather than fail wrong")?;

    let cr = ctx.collateral_ratio();
    let redeem = ctx.levercoin_redeem_nav().context("levercoin_redeem_nav")?;
    let mint = ctx.levercoin_mint_nav().context("levercoin_mint_nav")?;
    let stable = ctx.stablecoin_nav().context("stablecoin_nav")?;

    println!("observed slot   : {slot}");
    println!("epoch           : {epoch}");
    println!("collateral ratio: {cr:?}\n");

    println!("xSOL NAV");
    println!("  redeem (floor, price.lower)  = {}  <-- SVI quote_amount / lower", redeem);
    println!("  mint   (ceil,  price.upper)  = {}  <-- SVI upper", mint);
    println!("  hyUSD NAV                    = {}", stable);

    println!("\nCompare `redeem` against the xSOL price on https://hylo.so");
    println!("A match proves the thesis: this value is reproducible by anyone,");
    println!("from public state, with no API in the path.");
    Ok(())
}

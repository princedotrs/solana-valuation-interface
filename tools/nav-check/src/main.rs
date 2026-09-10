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
//!     cargo run -- --json            # machine-readable, for piping
//!
//! Then compare `redeem NAV` against the xSOL price shown on hylo.so.

use std::env;
use std::sync::Arc;

use anyhow::{Context, Result};
use hylo_core::exchange_context::ExchangeContext;
use hylo_quotes::protocol_state::RpcStateProvider;
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;

/// The `hylo-core` revision this binary was built against. Keep in sync with
/// the `rev` in Cargo.toml — it is the provenance of every number printed.
const SDK_REV: &str = "ee8d1cb";

/// Strip credentials from an RPC URL before it is ever printed.
///
/// This output is meant to be screenshotted, and the obvious RPC providers put
/// the API key in the query string (Helius) or the path (QuickNode, Alchemy).
/// Leaking a key into a screenshot is a real and common accident, so redact
/// rather than trusting the operator to have used a clean URL.
fn redact(url: &str) -> String {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));

    // Path segments that look like a key (long, hex-ish, or a UUID).
    let mut host_part = base.to_string();
    if let Some(scheme_end) = base.find("://") {
        let (scheme, rest) = base.split_at(scheme_end + 3);
        let mut segs: Vec<String> = rest.split('/').map(str::to_string).collect();
        for seg in segs.iter_mut().skip(1) {
            let looks_secret = seg.len() >= 16
                && seg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if looks_secret {
                *seg = "<redacted>".to_string();
            }
        }
        host_part = format!("{scheme}{}", segs.join("/"));
    }

    if query.is_empty() {
        return host_part;
    }
    let scrubbed: Vec<String> = query
        .split('&')
        .map(|kv| match kv.split_once('=') {
            Some((k, _)) => format!("{k}=<redacted>"),
            None => kv.to_string(),
        })
        .collect();
    format!("{host_part}?{}", scrubbed.join("&"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let url = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .or_else(|| env::var("RPC_URL").ok())
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());

    let rpc = Arc::new(RpcClient::new_with_commitment(
        url.clone(),
        CommitmentConfig::confirmed(),
    ));
    let slot = rpc.get_slot().await.context("get_slot")?;
    let epoch = rpc.get_epoch_info().await.context("get_epoch_info")?.epoch;

    // Four accounts, one call: Hylo protocol state, xSOL mint, Pyth SOL/USD,
    // and the Clock sysvar. All read at the same slot — the same-slot
    // consistency the on-chain adapter gets for free inside one transaction.
    let provider = RpcStateProvider::new(rpc);
    let ctx = provider.fetch_lst_context().await.context(
        "fetch_lst_context — if this failed with TotalSolCacheOutdated, a new \
         epoch began and Hylo's LST crank has not run yet. That is the CORRECT \
         failure: spec §5.1, fail stale rather than fail wrong",
    )?;

    let cr = ctx.collateral_ratio();
    let redeem = ctx.levercoin_redeem_nav().context("levercoin_redeem_nav")?;
    let mint = ctx.levercoin_mint_nav().context("levercoin_mint_nav")?;
    let stable = ctx.stablecoin_nav().context("stablecoin_nav")?;
    let supply = ctx.levercoin_supply().context("levercoin_supply")?;

    if json {
        println!(
            r#"{{"slot":{slot},"epoch":{epoch},"sdk_rev":"{SDK_REV}","xsol_nav_redeem":"{redeem}","xsol_nav_mint":"{mint}","hyusd_nav":"{stable}","xsol_supply":"{supply}","collateral_ratio":"{cr:?}"}}"#
        );
        return Ok(());
    }

    println!();
    println!("  xSOL NAV, recomputed from mainnet state");
    println!("  ─────────────────────────────────────────────────────────");
    println!();
    println!("    redeem NAV   $ {redeem}");
    println!("    mint NAV     $ {mint}");
    println!();
    println!("  ─────────────────────────────────────────────────────────");
    println!("    hyUSD NAV      $ {stable}");
    println!("    xSOL supply      {supply}");
    println!("    collateral ratio {cr:?}");
    println!();
    println!("    slot {slot}  ·  epoch {epoch}  ·  hylo-core @ {SDK_REV}");
    println!("    rpc  {}", redact(&url));
    println!();
    println!("  Four accounts. Hylo's own math, pinned to a commit.");
    println!("  No API, no private arithmetic, nobody's permission.");
    println!("  Compare `redeem NAV` to the xSOL price on https://hylo.so");
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::redact;

    /// This output is meant to be screenshotted and posted publicly, so a
    /// credential surviving into it is a real incident, not a nit.
    #[test]
    fn redacts_query_string_keys() {
        assert_eq!(
            redact("https://mainnet.helius-rpc.com/?api-key=20923543-60b6-4c69-ab08-478a5897daa3"),
            "https://mainnet.helius-rpc.com/?api-key=<redacted>"
        );
    }

    #[test]
    fn redacts_path_embedded_keys() {
        assert_eq!(
            redact("https://example.quiknode.pro/abc123def456ghi789jkl/"),
            "https://example.quiknode.pro/<redacted>/"
        );
    }

    #[test]
    fn leaves_clean_urls_alone() {
        assert_eq!(
            redact("https://api.mainnet-beta.solana.com"),
            "https://api.mainnet-beta.solana.com"
        );
    }
}

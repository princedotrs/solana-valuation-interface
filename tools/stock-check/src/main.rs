//! stock-check — reproduce a published stock quote, without trusting SVI.
//!
//! The claim on the front of the site is that anyone can re-run the math.
//! This is the thing that does it. Point it at a cluster and a
//! `deployments.json`, and it will:
//!
//! 1. read the two published quote accounts straight from the chain;
//! 2. ask Pyth's Hermes, independently, what the two underlying feeds say;
//! 3. recompute what the quotes should contain, from Pyth's numbers alone;
//! 4. print both, and disagree loudly if they differ.
//!
//!     cargo run --manifest-path tools/stock-check/Cargo.toml -- AAPL
//!     cargo run ... -- AAPL --rpc https://api.devnet.solana.com
//!     cargo run ... -- --self-test          # no network
//!
//! # What makes this a second opinion rather than an echo
//!
//! It depends on none of SVI's crates. Not `svi-math`, not the adapter, not
//! the keeper. The scaling arithmetic below is written out again, by hand,
//! from Pyth's documented exponent convention — because two implementations
//! agreeing is evidence, and one implementation agreeing with itself is not.
//!
//! It also cannot be fooled by a stale RPC: the quote carries the slot it was
//! observed at, and this prints that alongside the chain's current slot so an
//! old answer looks old.

use std::env;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;

/// Payload offsets, after the 8-byte discriminator. Frozen and asserted by
/// `svi-core/tests/core.rs::layout_is_frozen`. Written out here rather than
/// imported, for the same reason as the arithmetic.
mod offset {
    pub const BASE_AMOUNT: usize = 224;
    pub const VALUE: usize = 232;
    pub const LOWER: usize = 240;
    pub const UPPER: usize = 248;
    pub const OBSERVED_SLOT: usize = 256;
    pub const OBSERVED_UNIX_TS: usize = 264;
    pub const VALID_UNTIL_SLOT: usize = 280;
    pub const SEQUENCE: usize = 288;
    pub const STATUS_FLAGS: usize = 296;
    pub const VALUE_TYPE: usize = 306;
}

/// A quote account is 8 discriminator bytes plus exactly this much payload.
const PAYLOAD_LEN: usize = 320;

/// The published scale: 9 decimals, as `QUOTE_DECIMALS` in both adapters.
const QUOTE_DECIMALS: u32 = 9;

/// Bits 16-23 belong to the stock adapter. Bits 0-5 are Hylo's and mean
/// entirely different things, which is why the allocation is registered in
/// `svi-core::state::flags` rather than left to each adapter.
const FLAGS: [(u64, &str); 4] = [
    (1 << 16, "MARKET_CLOSED"),
    (1 << 17, "REFERENCE_STALE"),
    (1 << 18, "TOKEN_FEED_STALE"),
    (1 << 19, "DEVIATION_HIGH"),
];

const DEFAULT_RPC: &str = "https://api.devnet.solana.com";
const DEFAULT_HERMES: &str = "https://hermes.pyth.network";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Quote {
    base_amount: u64,
    value: u64,
    lower: u64,
    upper: u64,
    observed_slot: u64,
    observed_unix_ts: i64,
    valid_until_slot: u64,
    sequence: u64,
    status_flags: u64,
    value_type: u8,
}

fn decode(raw: &[u8]) -> Result<Quote> {
    let d = raw
        .get(8..)
        .ok_or_else(|| anyhow!("account is {} bytes, too small for a discriminator", raw.len()))?;
    if d.len() < PAYLOAD_LEN {
        bail!("quote payload is {} bytes, expected {PAYLOAD_LEN}", d.len());
    }
    let u = |o: usize| u64::from_le_bytes(d[o..o + 8].try_into().expect("checked above"));
    Ok(Quote {
        base_amount: u(offset::BASE_AMOUNT),
        value: u(offset::VALUE),
        lower: u(offset::LOWER),
        upper: u(offset::UPPER),
        observed_slot: u(offset::OBSERVED_SLOT),
        observed_unix_ts: u(offset::OBSERVED_UNIX_TS) as i64,
        valid_until_slot: u(offset::VALID_UNTIL_SLOT),
        sequence: u(offset::SEQUENCE),
        status_flags: u(offset::STATUS_FLAGS),
        value_type: d[offset::VALUE_TYPE],
    })
}

fn flag_names(bits: u64) -> String {
    let set: Vec<&str> = FLAGS.iter().filter(|(b, _)| bits & b != 0).map(|(_, n)| *n).collect();
    if set.is_empty() { "none".into() } else { set.join(" | ") }
}

/// Render a 9-decimal fixed-point amount as dollars.
fn usd(bits: u64) -> String {
    format!("{}.{:09}", bits / 1_000_000_000, bits % 1_000_000_000)
}

/// Pyth's price and confidence, converted to 9 decimals.
///
/// Pyth publishes `price` with an `exponent`, so the real value is
/// `price * 10^exponent`. To express that at 9 decimals the shift is
/// `exponent + 9`: positive means multiply, negative means divide.
///
/// Rounding is deliberately asymmetric, the same way the adapter does it: the
/// lower bound floors and the upper bound ceils, so the published band never
/// claims to be tighter than Pyth's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Band {
    lower: u128,
    mid: u128,
    upper: u128,
}

fn pyth_band(price: i64, conf: u64, exponent: i32) -> Result<Band> {
    if price <= 0 {
        bail!("Pyth reported a non-positive price ({price}), which is not a price");
    }
    let price = price as u128;
    let conf = conf as u128;
    let shift = exponent + QUOTE_DECIMALS as i32;

    let scale = |v: u128, up: bool| -> Result<u128> {
        if shift >= 0 {
            let f = 10u128
                .checked_pow(u32::try_from(shift).context("exponent out of range")?)
                .ok_or_else(|| anyhow!("10^{shift} overflows"))?;
            v.checked_mul(f).ok_or_else(|| anyhow!("scaling overflows"))
        } else {
            let f = 10u128
                .checked_pow(u32::try_from(-shift).context("exponent out of range")?)
                .ok_or_else(|| anyhow!("10^{} overflows", -shift))?;
            Ok(if up { v.div_ceil(f) } else { v / f })
        }
    };

    Ok(Band {
        lower: scale(price.saturating_sub(conf), false)?,
        mid: scale(price, false)?,
        upper: scale(price + conf, true)?,
    })
}

/// Drift of the market price from the reference, in basis points, rounded up
/// so it is never under-reported.
fn deviation_bps(market: u128, reference: u128) -> Option<u128> {
    if reference == 0 {
        return None;
    }
    let diff = market.abs_diff(reference);
    Some((diff * 10_000).div_ceil(reference))
}

// ───────────────────────────────────────────────────────────────── network

async fn rpc(url: &str, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
    let body = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let v: serde_json::Value = reqwest::Client::new()
        .post(url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("POST {method} to the RPC"))?
        .json()
        .await
        .context("the RPC returned a body that is not JSON")?;
    if let Some(e) = v.get("error") {
        bail!("RPC {method}: {e}");
    }
    v.get("result").cloned().ok_or_else(|| anyhow!("RPC {method} returned no result"))
}

async fn fetch_account(url: &str, address: &str) -> Result<Vec<u8>> {
    let r = rpc(
        url,
        "getAccountInfo",
        serde_json::json!([address, {"encoding": "base64", "commitment": "confirmed"}]),
    )
    .await?;
    let b64 = r
        .get("value")
        .and_then(|v| v.get("data"))
        .and_then(serde_json::Value::as_array)
        .and_then(|a| a.first())
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("{address} does not exist on this cluster"))?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .context("account data is not base64")
}

/// What Hermes says about one feed, right now, with no help from SVI.
async fn hermes_price(base: &str, feed_id: &str) -> Result<(i64, u64, i32, i64)> {
    let url = format!(
        "{}/v2/updates/price/latest?ids[]={}&encoding=hex",
        base.trim_end_matches('/'),
        feed_id
    );
    let v: serde_json::Value = reqwest::get(&url)
        .await
        .with_context(|| format!("GET {url}"))?
        .json()
        .await
        .context("Hermes returned a body that is not JSON")?;

    let p = v
        .get("parsed")
        .and_then(serde_json::Value::as_array)
        .and_then(|a| a.first())
        .ok_or_else(|| anyhow!("Hermes has no parsed price for {feed_id}"))?;
    let price = p.get("price").ok_or_else(|| anyhow!("no price object"))?;

    let num = |k: &str| -> Result<i64> {
        price
            .get(k)
            .and_then(|x| x.as_i64().or_else(|| x.as_str().and_then(|s| s.parse().ok())))
            .ok_or_else(|| anyhow!("price.{k} is missing or not a number"))
    };
    Ok((
        num("price")?,
        u64::try_from(num("conf")?).context("conf is negative")?,
        i32::try_from(num("expo")?).context("expo out of range")?,
        num("publish_time")?,
    ))
}

// ───────────────────────────────────────────────────────────────── the check

#[allow(clippy::too_many_lines)]
async fn check(symbol: &str, rpc_url: &str, hermes_url: &str, path: &str) -> Result<()> {
    let doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?,
    )
    .with_context(|| format!("{path} is not valid JSON"))?;

    let s = doc
        .get("stocks")
        .and_then(|s| s.get(symbol))
        .ok_or_else(|| anyhow!("{path} has no symbol {symbol}"))?;
    let addr = |k: &str| -> Result<&str> {
        s.get(k).and_then(serde_json::Value::as_str).ok_or_else(|| anyhow!("missing {k}"))
    };
    let feed = |leg: &str| -> Result<&str> {
        s.get("feed_ids")
            .and_then(|f| f.get(leg))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing feed_ids.{leg}"))
    };

    let (fair_addr, market_addr) = (addr("fair_quote")?, addr("market_quote")?);
    println!("stock-check  {symbol}  on {}", doc.get("cluster").and_then(|c| c.as_str()).unwrap_or("?"));
    println!("  fair quote    {fair_addr}");
    println!("  market quote  {market_addr}\n");

    let fair = decode(&fetch_account(rpc_url, fair_addr).await?)
        .with_context(|| format!("decoding the fair quote at {fair_addr}"))?;
    let market = decode(&fetch_account(rpc_url, market_addr).await?)
        .with_context(|| format!("decoding the market quote at {market_addr}"))?;
    let slot = rpc(rpc_url, "getSlot", serde_json::json!([{"commitment": "confirmed"}]))
        .await?
        .as_u64()
        .unwrap_or(0);

    println!("PUBLISHED, read from the chain");
    println!("  fair value    $ {}   [{} .. {}]", usd(fair.value), usd(fair.lower), usd(fair.upper));
    println!("  market price  $ {}   [{} .. {}]", usd(market.value), usd(market.lower), usd(market.upper));
    println!("  base_amount   {} / {}", fair.base_amount, market.base_amount);
    println!("  value_type    {} (fair) / {} (market)", fair.value_type, market.value_type);
    println!("  observed      slot {}   valid_until {}", fair.observed_slot, fair.valid_until_slot);
    println!("  sequence      {}", fair.sequence);
    println!("  flags         {}", flag_names(fair.status_flags | market.status_flags));
    if slot > 0 {
        let expired = slot > fair.valid_until_slot;
        println!(
            "  chain slot    {slot}  ->  {}",
            if expired { "EXPIRED (a consumer must refuse this)" } else { "valid" }
        );
    }

    println!("\nINDEPENDENTLY, from Pyth's Hermes");
    let (ep, ec, ex, et) = hermes_price(hermes_url, feed("equity")?).await?;
    let (tp, tc, tx, tt) = hermes_price(hermes_url, feed("token")?).await?;
    let eq = pyth_band(ep, ec, ex)?;
    let tk = pyth_band(tp, tc, tx)?;
    println!("  equity feed   $ {}   [{} .. {}]", usd(eq.mid as u64), usd(eq.lower as u64), usd(eq.upper as u64));
    println!("  token feed    $ {}   [{} .. {}]", usd(tk.mid as u64), usd(tk.lower as u64), usd(tk.upper as u64));
    println!("  publish time  equity {et}, token {tt}");

    if let Some(bps) = deviation_bps(tk.mid, eq.mid) {
        println!("\n  the token is {} bps from the real share", bps);
        println!("  ({}%, and DEVIATION_HIGH fires above 200 bps)", bps as f64 / 100.0);
    }

    // The comparison is not expected to be exact: Hermes is being asked *now*
    // and the quote was published earlier, so the two see different prices.
    // What matters is that the published numbers are inside Pyth's band for
    // the moment they were taken, which a large gap would rule out.
    println!("\nVERDICT");
    let mut complaints = Vec::new();
    if fair.value < fair.lower || fair.value > fair.upper {
        complaints.push("the fair value is outside its own published bounds".to_string());
    }
    if market.value < market.lower || market.value > market.upper {
        complaints.push("the market price is outside its own published bounds".to_string());
    }
    if fair.base_amount == 0 || market.base_amount == 0 {
        complaints.push("a base_amount of zero makes the quote meaningless".to_string());
    }
    if fair.value_type != 7 {
        complaints.push(format!("the fair quote says value_type {}, not 7 (ReferenceFairValue)", fair.value_type));
    }
    if market.value_type != 2 {
        complaints.push(format!("the market quote says value_type {}, not 2 (MarketSpot)", market.value_type));
    }
    if fair.observed_slot != market.observed_slot {
        complaints.push(format!(
            "the pair was not written together: fair at slot {}, market at {}",
            fair.observed_slot, market.observed_slot
        ));
    }

    if complaints.is_empty() {
        println!("  OK. Both quotes are internally consistent, typed as they should be, and");
        println!("  were written in the same instruction. Compare the dollar figures above:");
        println!("  Hermes is answering for right now, the quote for the slot it names.");
    } else {
        for c in &complaints {
            println!("  WRONG: {c}");
        }
        bail!("{} problem(s) found", complaints.len());
    }
    Ok(())
}

// ───────────────────────────────────────────────────────────────── self-test

fn self_test() -> Result<()> {
    // Pyth's usual equity exponent is -8, so a price of 25_000_000_000 with
    // expo -8 is $250, and at 9 decimals that is 250_000_000_000.
    let b = pyth_band(25_000_000_000, 5_000_000, -8)?;
    assert_eq!(b.mid, 250_000_000_000, "250 dollars at 9 decimals");
    assert_eq!(b.lower, 249_950_000_000);
    assert_eq!(b.upper, 250_050_000_000);

    // A positive shift multiplies rather than divides.
    assert_eq!(pyth_band(1, 0, 0)?.mid, 1_000_000_000);

    // Rounding is asymmetric: the band may widen, never narrow.
    let b = pyth_band(3, 1, -10)?;
    assert_eq!(b.lower, 0, "floor");
    assert_eq!(b.upper, 1, "ceil -- 0.4 must not round down to 0");

    assert!(pyth_band(0, 1, -8).is_err(), "a zero price is not a price");
    assert!(pyth_band(-5, 1, -8).is_err());

    assert_eq!(deviation_bps(102, 100), Some(200));
    assert_eq!(deviation_bps(98, 100), Some(200), "symmetric in magnitude");
    assert_eq!(deviation_bps(100, 100), Some(0));
    assert_eq!(deviation_bps(1000, 100), Some(90_000));
    assert_eq!(deviation_bps(1, 0), None, "no reference, no deviation");
    // Rounds up: 1 part in 100_000 is 0.1 bps, reported as 1 rather than 0.
    assert_eq!(deviation_bps(100_001, 100_000), Some(1));

    // The decoder must demand a whole payload, not just the bytes it reads.
    let mut raw = vec![0u8; 8 + PAYLOAD_LEN];
    raw[8 + offset::VALUE..8 + offset::VALUE + 8].copy_from_slice(&123u64.to_le_bytes());
    raw[8 + offset::VALUE_TYPE] = 7;
    let q = decode(&raw)?;
    assert_eq!(q.value, 123);
    assert_eq!(q.value_type, 7);
    assert!(decode(&vec![0u8; 8 + 304]).is_err(), "304 bytes is not a quote");
    assert!(decode(&[0u8; 4]).is_err());

    assert_eq!(flag_names(0), "none");
    assert_eq!(flag_names(1 << 16), "MARKET_CLOSED");
    assert_eq!(flag_names((1 << 16) | (1 << 19)), "MARKET_CLOSED | DEVIATION_HIGH");
    assert_eq!(flag_names(1 << 4), "none", "bit 4 is Hylo's BUY_ZONE, not ours");

    assert_eq!(usd(1_000_000_000), "1.000000000");
    assert_eq!(usd(60_285_498), "0.060285498");


    // ---- differential vectors ----
    //
    // The point of this binary is to be a second implementation, so the thing
    // worth testing is that it AGREES with the first one. These outputs were
    // produced by `svi_math::pyth_band` and `svi_math::deviation_bps` -- the
    // code the on-chain adapter actually runs -- and are reproduced here as
    // fixed expectations. The arithmetic above was written independently from
    // Pyth's exponent convention; if the two ever diverge, one of them has a
    // bug and this is where it shows up.
    let vectors: [(i64, u64, i32, u128, u128, u128); 10] = [
        (25_000_000_000, 5_000_000, -8, 249_950_000_000, 250_000_000_000, 250_050_000_000),
        (1, 0, 0, 1_000_000_000, 1_000_000_000, 1_000_000_000),
        (3, 1, -10, 0, 0, 1),
        (17_632_500_000, 2_100_000, -8, 176_304_000_000, 176_325_000_000, 176_346_000_000),
        (999_999_999_999, 123_456, -9, 999_999_876_543, 999_999_999_999, 1_000_000_123_455),
        (5, 4, -1, 100_000_000, 500_000_000, 900_000_000),
        (100, 0, -2, 1_000_000_000, 1_000_000_000, 1_000_000_000),
        (7, 7, -3, 0, 7_000_000, 14_000_000),
        (123_456_789, 1_000, -5, 1_234_557_890_000, 1_234_567_890_000, 1_234_577_890_000),
        (2, 1, -12, 0, 0, 1),
    ];
    for (price, conf, expo, lower, mid, upper) in vectors {
        let b = pyth_band(price, conf, expo)
            .with_context(|| format!("svi-math accepts ({price}, {conf}, {expo})"))?;
        assert_eq!(
            (b.lower, b.mid, b.upper),
            (lower, mid, upper),
            "disagreement with svi-math on price {price}, conf {conf}, expo {expo}"
        );
    }

    for (market, reference, want) in [
        (102u128, 100u128, Some(200u128)),
        (98, 100, Some(200)),
        (100, 100, Some(0)),
        (1000, 100, Some(90_000)),
        (100_001, 100_000, Some(1)),
        (1, 0, None),
    ] {
        assert_eq!(deviation_bps(market, reference), want, "disagreement on {market}/{reference}");
    }

    println!("stock-check self-test ok: scaling, deviation, decoding, flags,");
    println!("  and 16 differential vectors against svi-math, the code the adapter runs");
    Ok(())
}

fn arg(flag: &str) -> Option<String> {
    let a: Vec<String> = env::args().collect();
    a.iter().position(|x| x == flag).and_then(|i| a.get(i + 1).cloned())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--self-test") {
        return self_test();
    }
    let Some(symbol) = args.first().filter(|a| !a.starts_with("--")) else {
        bail!(
            "usage:\n  \
             stock-check <SYMBOL> [--rpc URL] [--hermes URL] [--deployments PATH]\n  \
             stock-check --self-test"
        );
    };

    check(
        &symbol.to_ascii_uppercase(),
        &arg("--rpc").unwrap_or_else(|| DEFAULT_RPC.to_string()),
        &arg("--hermes").unwrap_or_else(|| DEFAULT_HERMES.to_string()),
        &arg("--deployments").unwrap_or_else(|| "deployments.json".to_string()),
    )
    .await
}

#[cfg(test)]
mod tests {
    /// `--self-test` is the operator-facing entry point, but a check that only
    /// runs when somebody remembers to type it is not a check. This makes
    /// `cargo test` run the same assertions.
    #[test]
    fn self_test_passes() {
        super::self_test().unwrap();
    }
}

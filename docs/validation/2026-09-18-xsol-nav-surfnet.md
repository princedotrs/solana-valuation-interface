# Validation record — xSOL NAV published on-chain, and independently agreed

**Date:** 2026-09-18 · **Slot:** 448083727 · **Epoch:** 1037
**Status:** ✅ **Published by the program, matched by an off-chain reader to the last digit.**
**Run:** `cargo test --test surfnet -- --nocapture` (svi-hylo-adapter)
**Where:** a Surfnet forking mainnet · **Math:** `hylo-core` @ `ee8d1cb`

The 10 September record showed the NAV could be *recomputed* from public state.
This one shows it *published*: an on-chain program derived the number itself,
wrote it into an `svi-core` quote account, and an independent reader sharing no
code path arrived at the same value.

---

## Result

| Quantity | Value |
|---|---|
| Quote account | `2EhQfH26gQBxi8RteUVCXXA5MMnwuLZgFnMqQxLkseuc` |
| `quote_amount` (redeem side) | **$0.069715188** |
| `lower` .. `upper` | $0.069715188 .. $0.069809602 |
| Band width | 0.000094414 — **13.54 bps** |
| `base_amount` | 1000000 (1 whole xSOL) |
| `observed_slot` / `valid_until_slot` | 448083727 / 448084477 (750 slots ≈ 5 min) |
| `sequence` | 1 |
| `status_flags` | `0b10000` = `BUY_ZONE` |
| Off-chain `hylo-core` | **$0.069715188** — identical |

`BUY_ZONE` is Hylo's own protocol state reported through the quote, not a
fault: the collateral ratio sits where minting xSOL is favourable. A consumer
reads it and decides; the adapter neither suppresses nor interprets it.

## Programs

| | |
|---|---|
| `svi-core` | `5yVpoQJCtEqM1RC2Fn4D6rCSoq79e3tQ92Kx4fyePECN` |
| `svi-hylo-adapter` | `FfK5xyE3v3GT2F9wzqhpLMHx7zCJsGPQrHAvxtgL3Mpr` |

Both deployed from the binaries built in that run, at the ids their binaries
declare. Transactions: `initialize_feed` `4JBPgCD2…`, `initialize`
`GfMaUsgd…`, `refresh_xsol_nav` `k4uKtVrZ…`.

## What was and was not simulated

Hylo's three accounts were re-cloned from mainnet immediately before the
refresh, and the Surfnet clock was pinned to the snapshot's own slot and
timestamp for the single instruction that reads it. **No Hylo or Pyth byte was
altered.** Re-cloning is what a fresh fork does; pinning evaluates the refresh
exactly as a fork taken at slot 448083727 would.

The pinning is necessary because Hylo's oracle window is ten seconds and
`hylo-core` requires both `posted_slot <= slot <= posted_slot + 25` and
`unix_timestamp <= publish_time + 10`. A Surfnet ticks at a fixed 400 ms while
its clock tracks wall time, so on any fork older than a few minutes the two
cannot both hold. At the pinned slot the Pyth update was 1 second and 1 slot
old — comfortably inside Hylo's own tolerance.

**This ran on a fork, not on mainnet or devnet.** Hylo is a mainnet protocol,
so its state does not exist on devnet and this adapter cannot be meaningfully
deployed there. Say "a mainnet fork" and mean it.

## Why the match matters

The off-chain figure comes from `hylo-core` called directly by the test, using
`offchain` conversions rather than the adapter's `idl_bridge`. The two paths
share the maths library and nothing else — not the account decoding, not the
scaling, not the rounding direction. Agreement to the ninth decimal is evidence
the adapter's own plumbing introduces no drift.

## What this does not establish

- No consumer reads this feed.
- No third-party audit. Nothing should be collateralised against it.
- One symbol, one refresh, one fork. Not a service-level claim.

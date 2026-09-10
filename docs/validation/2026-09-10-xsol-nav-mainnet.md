# Validation record — xSOL NAV reproduced from mainnet

**Date:** 2026-09-10 · **Slot:** 445910996 · **Epoch:** 1032
**Tool:** [`tools/nav-check`](../../tools/nav-check) · **Math:** `hylo-core` @ `ee8d1cb`

The first end-to-end evidence for SVI's central claim: the value of a
protocol-derived asset can be recomputed by anyone, from public account state,
with no API and no cooperation from the issuing protocol.

---

## What was run

Four accounts, fetched in a single `getMultipleAccounts` at one slot: Hylo's
protocol state, the xSOL mint, the Pyth SOL/USD feed, and the Clock sysvar.
The NAV was then computed by `hylo-core` — Hylo's own published math library,
pinned to an exact commit. No formula was reimplemented.

## Result

| Quantity | Value |
|---|---|
| xSOL NAV, redeem side (floor math, `price.lower`) | **$0.061326271** |
| xSOL NAV, mint side (ceil math, `price.upper`) | **$0.061368725** |
| hyUSD NAV | $1.000000000 |
| xSOL supply | 166,267,548.251798 |
| Collateral ratio | 1.661501243 (166.15%) |

Redeem NAV is the number a consumer uses: it is what a holder liquidating xSOL
actually realises, and therefore the defensible collateral value.

## Independent cross-checks

These do not depend on Hylo's front end. They test whether the reported numbers
are mutually consistent under Hylo's own equations — which they could not be if
a decimal place, a rounding direction, or an account had been read wrongly.

**1. The uncertainty band is a plausible oracle confidence interval.**
Spread of $0.000042454, or **6.92 bps** of the midpoint. Wide enough to be a
real Pyth confidence interval, narrow enough to be usable as collateral.

**2. NAV, supply and collateral ratio agree with each other.**
`hylo-core` defines `CR = collateral_value / vUSD_supply` and
`NAV = (collateral_value − vUSD_value) / xSOL_supply`. Both use `price.lower`,
so they share a price basis and must reconcile. Solving for pool state using
only the three reported numbers:

| Back-solved from NAV, supply and CR alone | |
|---|---|
| xSOL market cap | $10,196,568 |
| vUSD, SOL pool | $15,414,285 |
| SOL-pool TVL | $25,610,853 |
| **Implied leverage** (TVL ÷ xSOL market cap) | **2.51×** |

The leverage figure is the strongest signal in this record. Nothing put it
there: it falls out of three independently-read on-chain quantities, and it
lands where a leveraged SOL token should. A misread account or a decimals error
would not produce a coherent multiple — it would produce a number off by orders
of magnitude.

**3. Status flags are self-consistent.** CR is comfortably above 100%, so the
protocol is not in the depeg zone, and hyUSD reports exactly $1.000000000
rather than a depeg NAV. Those two facts have to agree, and do.

## Second observation — the strongest check available

A second run 3,810 slots (~25 minutes) later, at **slot 445914806**, turns a
single reading into a two-point test. Back-solving pool state independently at
each slot:

| | slot 445910996 | slot 445914806 | change |
|---|---|---|---|
| xSOL NAV (redeem) | $0.061326271 | $0.061469035 | +0.2328% |
| xSOL market cap | $10,196,568.72 | $10,219,515.51 | +0.2251% |
| **vUSD, SOL pool** | **$15,414,285.05** | **$15,414,284.97** | **−0.0000%** |
| SOL-pool TVL | $25,610,853.78 | $25,633,800.48 | +0.0896% |
| Implied leverage | 2.5117× | 2.5083× | — |

**The vUSD row is the result.** Two independent reads, twenty-five minutes
apart, each back-solved through three separately-fetched quantities — and the
implied hyUSD supply agrees to **eight cents on $15.4 million**, one part in
190 million. Nothing forces that. If the NAV computation, the supply decoding,
or the collateral-ratio read were wrong in any way that mattered, the two
solutions would diverge. They do not, because hyUSD supply genuinely barely
moves over 25 minutes while SOL's price does.

**xSOL amplified its collateral by 2.60×** over the interval, against a
reported leverage of 2.51× — 3.4% apart. Exact agreement is not expected:
leverage is a ratio that shifts as price moves, and xSOL supply changed by
−0.0077% (mints and burns) during the window. Agreement to within a few percent
over a 9-basis-point collateral move is what a correctly-behaving leveraged
token looks like.

## Ruling out a decimals error, quantitatively

Hylo's front end charts SOL/USD around **$103.60** at this time. That is not
xSOL, and cannot be:

| xSOL priced at | implied market cap |
|---|---|
| $103.60 | **$17,225,317,999** |
| $0.061326271 | $10,196,569 |

Solana's entire DeFi TVL is roughly $12B. A single leveraged token on a $25M
pool cannot be worth $17.2B. Conversely, at SOL ≈ $103.50 the back-solved TVL
implies the pool holds about **247,500 SOL** — an ordinary size for an LST pool
and consistent with everything else here.

## What this establishes

- The arithmetic thesis holds. The value is derivable from public state by a
  third party, using the protocol's own math, with nobody's permission.
- Spec §2's rounding table is correct as written: redeem uses floor against the
  lower price bound, mint uses ceil against the upper.
- The band is narrow enough that publishing bounds is useful rather than
  vacuous.

## What this does NOT establish

- **Nothing about the on-chain program.** This ran off-chain through one RPC.
  It is the arithmetic thesis, not the trust thesis. Same-slot atomic reads,
  adapter-PDA-only writes and the canonical quote account all live in
  `programs/`, and none of that is exercised here.
- **One RPC, one read.** A single provider was trusted for this run. The
  production design requires two independent providers in agreement (spec §8);
  that observer does not exist yet.
- **Not a comparison against Hylo's published xSOL figure.** The chart
  inspected showed SOL/USD, not xSOL. The checks above are internal-consistency
  and cross-slot tests, which are strong but are not the same as matching the
  number Hylo itself displays for xSOL. That comparison is still outstanding
  and should be recorded here when done.

## Reproducing

```bash
cd tools/nav-check
cargo run -- '<your-rpc-url>'
```

Numbers will differ — NAV moves with SOL. What should hold on any run is the
structure: redeem ≤ mint, a narrow band, and a leverage multiple that
reconciles with the collateral ratio.

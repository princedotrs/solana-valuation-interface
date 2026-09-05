# Proposal to Hylo

**Solana Valuation Interface × Hylo** · design-partner proposal · September 2026
**From:** Prince (SVI) · **To:** Plish and the Hylo team

---

## The ask, up front

Three things. None of them is engineering resource.

1. **Answer five technical questions** (§10 of the methodology spec, restated in §6 below). Roughly one engineer-afternoon.
2. **Let SVI name Hylo as design partner** for the methodology spec and the Colosseum submission.
3. **Co-announce the feeds** when they go live on mainnet.

**What Hylo is *not* being asked for:** no program changes, no engineering
sprint, no integration work, no funding, no exclusivity, no commitment to
consume anything.

**The deadline that matters:** the Colosseum hackathon runs **28 Sep – 2 Nov
2026**. Answers by mid-September make a mainnet-live submission with Hylo named
as partner realistic. Later is still useful; it just costs the funding window.

---

## 1. Why you, specifically

On 28 August you wrote:

> *"honestly an accepted smart contract interface for pricing onchain assets
> would be amazing — bc pyth wants us to read our own protocol by RPC and give
> them data by an API — it leaves all sorts of unimaginable security holes"*

You identified the problem unprompted and correctly. This proposal is the
thing you described, built, with Hylo as the first adapter.

The reason Hylo is the right first protocol is not the relationship. It is that
**xSOL's value is mathematically defined**. Unlike a memecoin or an LP token,
there is a right answer, it is computable from on-chain state, and `hylo-core`
already contains the exact function that computes it. That makes it the one
asset class where "here is a number you can independently reproduce" is a claim
that can actually be kept.

---

## 2. What you'd otherwise be signing up for

The pipeline Pyth asked for, drawn honestly:

```
Hylo accounts → one RPC → private parser → private math
              → Hylo-operated API → publisher key → on-chain price
```

Five trusted hops, and no way for a downstream consumer to check any of them.
Concretely, that means Hylo would own and operate:

- A server whose compromise is a mispricing of your own assets.
- Calculation code that nobody downstream can reproduce — so a bug in it is
  indistinguishable, from the outside, from a real market move.
- A permanent operational obligation with an uptime expectation attached, on a
  team that should be shipping V2.
- The reputational and plausibly legal exposure when a lender liquidates
  someone against a number your API produced.

None of that risk is compensated. It's pure liability accepted to unblock
someone else's integration.

---

## 3. What SVI does instead

```
Hylo accounts → adapter program (public code, on-chain integer math)
              → canonical quote account
              → mirror API (zero arithmetic, byte-verifiable)
              → publisher key → on-chain price
```

The arithmetic moves on-chain. The API becomes a photocopier: it reads a
canonical account through two independent RPCs, requires byte-for-byte
agreement, and serves the decoded value with provenance. It cannot invent a
number, because anyone can compare its output to the account it copied.

**Five trusted hops become one — and that one is Pyth's own publisher key,
which SVI does not fix and does not claim to.**

### Four properties worth calling out

**1. SVI reimplements none of your math.** The adapter depends on `hylo-core`,
pinned to an exact git SHA recorded on-chain. It calls
`exchange_math::next_levercoin_redeem_nav` and `next_levercoin_mint_nav`
directly. Your redemption math and SVI's published value cannot diverge,
because they are the same code.

**2. It inherits your safety semantics rather than inventing its own.**
`TotalSolCacheOutdated` already makes `hylo-core` refuse to compute across an
epoch boundary — SVI adopts that failure, it doesn't paper over it. Oracle
tolerance is read fresh from *your* on-chain `OracleConfig` every refresh; SVI
imposes no second opinion about your risk parameters. When CR < 100%, the feed
publishes **zero** with `DESTABILIZED | OPERATIONS_HALTED`, because that is
what your protocol says xSOL is worth.

**3. The uncertainty band maps onto your existing convention.** You already
price NAV as a range and always take the side favourable to the protocol —
mint uses ceil math with Pyth's upper bound, redeem uses floor with the lower.
SVI publishes `lower = redeem NAV`, `upper = mint NAV`, and headline
`quote_amount = redeem NAV`, because a holder liquidating xSOL realises the
redeem side. That is the defensible collateral value, and it's your number,
not ours.

**4. Zero changes to your programs, ever.** Solana programs have no view
functions and SVI doesn't need any. The adapter is handed your accounts and
deserializes them itself, verifying owner, PDA derivation and mint before
touching the bytes. The one CPI is to `update_lst_prices`, which is
permissionless (payer-only signer) — so even that needs no permission.

---

## 4. What Hylo gets

| | |
|---|---|
| **Zero engineering cost** | No program changes, no integration, no infrastructure to run |
| **The Pyth request answered** | Pyth gets exactly the API shape they asked for, minus the security objection you raised |
| **Liability moved off your team** | The valuation code is public, reproducible, and not yours to operate |
| **A feed lenders can actually underwrite** | Conservative redemption-anchored value, explicit bounds, freshness stamp, and a `DESTABILIZED` flag that unambiguously means *zero collateral value* — the things a risk lead needs to list xSOL |
| **Wider distribution for Hylo assets** | hyUSD is already on Kamino, Loopscale and Titan. A defensible NAV feed lowers the barrier for the next listing |
| **V2 scales down this path** | The xAsset Engine turns 4 feeds into 4 × N. A methodology-per-asset framework absorbs that; a hand-rolled API does not |
| **A public artifact that reflects well on Hylo** | `hylo-xsol-nav-v1` is a rigorous, public specification of Hylo's valuation semantics — useful documentation in its own right |
| **Optional co-operation of the keeper** | Cranking is permissionless. SVI runs a keeper during the pilot; Hylo can co-run one so liveness never depends on us |

---

## 5. What's already built

This is not a pitch for something that might exist. Current state of
[the repository](https://github.com/princedotrs/solana-valuation-interface):

| Component | State | What it is |
|---|---|---|
| `crates/svi-math` | ✅ **Done** | Exact integer math. `u128` intermediates, mandatory explicit rounding direction on every division, `no_std`, `#![deny(clippy::arithmetic_side_effects)]`, every function total (returns `Option`, never panics), property-tested |
| `programs/svi-core` | ✅ **Done** | Descriptor + a 320-byte zero-copy `Quote` with a compile-time layout assertion. `publish_quote` enforces: feed active, adapter PDA signed, `lower ≤ value ≤ upper`, observation not from the future, not over-age, strictly newer than stored. Emits `QuotePublished` |
| `programs/svi-mock-adapter` | ✅ **Done** | Exercises the CPI + authority path end to end |
| `docs/hylo-xsol-nav-v1-spec.md` | 🟡 **Draft v0.9** | The full methodology: accounts, formulas, rounding, edge cases, failure states, output mapping, update policy, verification procedure |
| `programs/svi-hylo-adapter` | 🔒 **Scaffolded** | Blocked on the five answers below |

The adapter is the only piece not written, and it is blocked on §6, not on
time.

---

## 6. The five questions

These are §10 of the methodology spec. Each one changes what gets built, which
is why they're being asked before rather than after.

### Q1 — V1 vs V2 layout, and timing
The equations doc describes V2; the addresses page lists V1 programs; the SDK
contains exo/cbBTC/HYPE machinery. **Which layout is live on mainnet today, and
is any account structure in spec §3 changing before Nov 2026?**

*Why it matters:* the adapter verifies exact PDA derivations and account
layouts. Building against V1 and shipping into V2 means shipping something that
aborts on day one — which is safe, but useless.

### Q2 — `update_lst_prices` via CPI
**Is there any constraint that prevents invoking `update_lst_prices` via CPI
from a third-party program** — LUT requirements, remaining-accounts size,
anything?

*Why it matters:* spec §5.1 bundles your permissionless LST crank into the
first refresh of each epoch. If that can't be CPI'd, the epoch-boundary path
becomes "abort and wait for someone else to crank", which is still safe but
degrades the feed for a window at every epoch rollover.

### Q3 — Pyth's exact request
**Which feeds, what cadence, and what response schema did Pyth actually ask you
for?**

*Why it matters:* this is the highest-value answer of the five. It defines the
mirror API contract and the keeper's refresh cadence. If Pyth asked for
xSOL/USD and hyUSD/USD every 10 seconds in a specific publisher format, and SVI
shows up with an eHYUSD rate every 5 minutes in its own schema, they'll shrug
and wait for your API — and the single best adoption story dies on a formatting
mismatch. A forwarded message or screenshot is enough.

### Q4 — SDK release pinning
**Will Hylo tag SDK releases aligned to program deployments**, so the adapter
can pin `hylo-core` to the deployed program version?

*Why it matters:* spec §2 pins to an exact git SHA. Tagged releases aligned to
deployments turn "pin a commit and hope" into a verifiable correspondence
between the math SVI runs and the program actually deployed. Without it, SVI
falls back to storing your programdata hash and aborting on any change —
workable, noisier.

### Q5 — Design-partner blessing
**May SVI list Hylo as design partner for the methodology spec and the
Colosseum submission, and would Hylo co-announce the feed when it goes live?**

*Why it matters:* a methodology spec the protocol has reviewed is worth
categorically more than one it hasn't. This is the difference between "someone
built a thing that reads Hylo" and "Hylo's valuation semantics, specified."

---

## 7. Pilot plan

Assumes answers by mid-September.

| Phase | Duration | What ships | Hylo's involvement |
|---|---|---|---|
| **0 · Spec freeze** | 1 week | Q1–Q5 folded in; methodology hashed and frozen at v1.0 | One review pass |
| **1 · Devnet** | 2 weeks | Adapter computes xSOL NAV on devnet; differential test vs `hylo-core` over 1,000+ randomized states; all failure paths tested | **None** |
| **2 · Observer parity** | 1 week | Independent observer reaches **zero** divergence over 72h continuous operation; compute budget measured < 200k CU | **None** |
| **3 · Mainnet feeds** | 1 week | `hylo-xsol-nav-v1`, `hylo-hyusd-backing-v1`, `hylo-ehyusd-rate-v1`, `hylo-xsol-leverage-v1` live; keeper running; quote addresses published | Optional: co-run a keeper |
| **4 · Mirror + Pyth** | 1 week | Mirror API in Pyth's requested schema, doing zero arithmetic; conformance suite public; Pyth evaluating | An intro to Pyth, if you're willing |
| **5 · Co-announce** | — | Public launch | A retweet and a line of confirmation |

**Total: ~6 weeks from spec freeze to Pyth evaluating.** Hylo's cumulative time
commitment across all six phases is measured in hours.

### Exit conditions

Stated so this can't quietly become an obligation:

- If Hylo wants the feeds retired, `set_feed_status` deprecates them, publicly, immediately, no negotiation.
- If Hylo builds its own equivalent, SVI's adapter is open source and Hylo is welcome to fork it, run it, or take it over outright.
- If Hylo declines to be named, SVI proceeds without the name and without the co-announcement. No hard feelings.
- Nothing here creates exclusivity in either direction.

---

## 8. Fair objections

The ones worth raising, answered honestly.

**"This makes SVI a dependency for our asset pricing."**
It doesn't, and this is the most important answer here. Cranking is
permissionless — Hylo (or anyone) can run a keeper. The adapter is open source.
The quote account is readable by anyone with no SVI infrastructure involved.
The methodology is a public document. If SVI disappears tomorrow, everything
except the mirror API keeps working, and the mirror is the piece that does no
math. That is deliberate: a valuation layer with a single operator is the thing
we're replacing, and it would be absurd to reintroduce it.

**"What if your adapter is wrong and someone gets liquidated?"**
A real risk, and the reason for four specific design decisions: SVI calls
`hylo-core` rather than reimplementing it; an independent observer recomputes
from raw accounts and halts the mirror on a divergence of even one unit; every
unverified input aborts rather than publishes; and real-collateral use is gated
on a third-party audit ($30–80k, engaged when a lender commits). Before that
audit, SVI's own position is that no lending market should underwrite against
these feeds — and that should be said publicly, not buried.

**"We're busy shipping V2."**
Which is the argument for this rather than against it. The alternative is your
team building and operating the API. This asks for an afternoon.

**"Why not just let Pyth do it?"**
Pyth can't — not for this asset class. Their model is publishers observing
markets. There is no market observation that yields xSOL NAV; there's only the
formula over your state. Someone has to run it. The question is only whether it
runs somewhere reproducible.

**"What's in it for you?"**
Directly: reputation, a Colosseum submission with a real design partner, and a
credible shot at authoring a Solana standard. Not revenue — the standard and
the SDKs are open source and permanently free, and there's no token. If SVI
ever charges anyone it'll be for managed keeper infrastructure and custom
adapters, not for this. Worth being blunt about, because an unexplained
motivation is its own kind of risk.

**"Is this just a hackathon project that dies in November?"**
Fair concern and the honest answer is: partly, that's what the funding path is
for. Mitigations that are real regardless — everything is open source, the
keeper is permissionless, the core program is small enough to freeze, and the
methodology spec stands on its own as documentation of Hylo's valuation
semantics even if every line of SVI code were abandoned.

---

## 9. What happens next

1. **You:** answer Q1–Q5 — Telegram, a call, or comments on the spec, whichever is least effort.
2. **Us:** fold the answers in, freeze `hylo-xsol-nav-v1` at v1.0, publish the hash.
3. **Us:** build, test, ship to devnet, run the observer to parity.
4. **You:** one review pass on the frozen spec, and a yes/no on being named.
5. **Us:** mainnet feeds, mirror API, Pyth intro.
6. **Both:** co-announce.

The methodology spec is [`docs/hylo-xsol-nav-v1-spec.md`](../hylo-xsol-nav-v1-spec.md).
It's complete except for the five answers. Reading §§2–6 is about fifteen
minutes and is the fastest way to check whether SVI has understood Hylo's math
correctly — which, if it hasn't, is the most valuable thing you could tell us.

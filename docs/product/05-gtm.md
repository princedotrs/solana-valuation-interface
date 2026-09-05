# SVI — Go-To-Market

**Solana Valuation Interface** · September 2026

> **The strategic bet in one paragraph.** SVI does not win by selling a
> product. It wins by becoming the thing Solana points at when someone asks
> "how should a protocol-derived value be published?" — and the only way to
> earn that is to have live mainnet feeds, a named design partner, and a
> second protocol that adopted it voluntarily, *before* the Foundation
> finishes standardising the neighbouring layer. Everything below is
> sequenced to reach that state in one quarter with one person.

---

## 1. Positioning

**For** protocols whose assets have a *value* rather than a *price* — leveraged
tokens, vault shares, LSTs, receipt tokens, tokenized funds —
**who** are being asked by oracles and lenders to hand over a number computed
in a private server,
**SVI is** an on-chain valuation interface
**that** computes the value from verified account state inside a Solana
transaction and publishes it to a canonical account anyone can read and
independently reproduce.
**Unlike** an off-chain pricing API or a TEE-executed oracle job,
**SVI** puts the arithmetic itself on-chain, so RPCs, APIs and publisher keys
are demoted from authors of the number to couriers of it.

**The one-liner:** *the value is computed on-chain and stored in an account —
so nobody has to trust an API doing hidden math.*

**The line for a lender:** *NAV you can recompute yourself, with a freshness
stamp, that says "zero" out loud when the protocol is underwater.*

### What SVI is not

Saying this early and unprompted is worth more than any feature claim,
because every one of these is what a skeptical listener assumes you mean:

- Not a Pyth competitor. Pyth is customer #1. SVI has nothing to say about SOL/USD.
- Not a new oracle network. No token, no operators, no slashing, ever.
- Not asking any protocol to change a single line of their code.
- Not a company with a revenue model yet — a standard with a services layer.

---

## 2. The market is three-sided, and only one side is hard

| Side | Who | What they must do | Difficulty |
|---|---|---|---|
| **Publish** | Hylo, then LSTs, vaults, Meteora, Kamino kTokens | **Nothing.** SVI writes adapters that read their public account bytes | ✅ Solved by architecture |
| **Consume** | Pyth, Kamino Scope, lending markets, wallets | Read an account, or an endpoint | ⚠️ **The whole game** |
| **Legitimise** | Solana Foundation (sRFC), auditors, Colosseum | Recognise the interface | ⚠️ Slow, compounding |

The single most important consequence of the architecture: **publisher-side
adoption cost is zero, so 100% of the risk is on the consume side.** GTM is
therefore consumer-first, and every publisher adapter exists to make a
consumer's life easier, not the other way round.

---

## 3. ICP and segmentation

### Publisher segments (ranked by how clean the math is)

| Tier | Segment | Why | Examples |
|---|---|---|---|
| **1** | **Leveraged / structured tokens** | NAV is a documented formula over on-chain state; no market price exists; consumers desperately need a defensible number | **Hylo xSOL**, xBTC, future xAssets |
| **1** | **Stablecoin backing NAV** | Deterministic, and the failure mode (depeg) is exactly when a market price is least trustworthy | **hyUSD** |
| **2** | **Vault / earn-pool shares** | Simple exchange-rate math; large TVL | sHYUSD, Kamino vaults, Meteora vaults |
| **2** | **LSTs** | Stake-pool redemption rate is trivially deterministic; enormous TVL (~$15B on Solana) | jitoSOL, hyloSOL, INF |
| **3** | **Lending receipt tokens** | Needs the CPI-to-accrue pattern; proves the harder half of the interface | kTokens, marginfi deposits |
| **3** | **LP positions** | Manipulable if done naively — only via the pool's own observation/TWAP accounts, tagged `MARKET_TWAP` | Meteora DAMM v2 / DLMM, Raydium CLMM |
| **4** | **Tokenized funds / RWA** | Fastest-growing segment, but often admin-attested rather than derivable — a partial fit, and where sRFC 40 and Chainlink both live | Solana RWA issuers |

**Deliberate sequencing note:** tier 1 first because the values are
*unambiguous*. A single wrong number in the first six months ends the project,
so early adapters must be ones where correctness is provable rather than
arguable.

### Consumer segments (ranked by ease of yes)

| Priority | Consumer | The ask | Why they say yes | Why they say no |
|---|---|---|---|---|
| **1** | **Pyth** | Consume a mirror endpoint | It is literally what they asked Hylo for, at zero integration cost, minus the security objection | They may just wait for Hylo's own API |
| **2** | **A lending market already listing Hylo assets** (Loopscale, Kamino, marginfi) | Read one quote account | They already carry the valuation risk and have no clean source. Loopscale has already lost $5.8M to exactly this class of bug | Integration risk on an unaudited feed |
| **3** | **Kamino Scope** | Add an `SVI quote` oracle type | One new type covers *every* future SVI feed instead of one per asset | They may prefer to keep valuation in-house |
| **4** | **Wallets / dashboards** | Display NAV honestly | Free, easy, and gives SVI public surface area | Low priority for them |

---

## 4. The wedge — sequenced, with a gate at each step

Each stage must produce a *checkable artifact* before the next begins. This is
what keeps a one-person project from generalising too early.

```
Stage 0  ▸  Methodology spec, co-signed         ← WE ARE HERE
Stage 1  ▸  Hylo adapter live on devnet, observer at parity
Stage 2  ▸  Mainnet feeds + verifiable mirror, Pyth consuming
Stage 3  ▸  First on-chain consumer reads a quote account
Stage 4  ▸  Adapter #2 — the interface proves it generalises
Stage 5  ▸  Into the sRFC process, not parallel to it
```

| Stage | Gate to pass before moving on | Timing |
|---|---|---|
| **0 — Spec** | Hylo answers spec §10 Q1–Q5; methodology frozen and hashed; Hylo agrees to be named design partner | **Now — this is the entire ask** |
| **1 — Devnet** | Adapter publishes `hylo-xsol-nav-v1`; observer reaches **zero** divergence over 72h; CU measured < 200k | ~3 weeks after Stage 0 |
| **2 — Mainnet + mirror** | 4 feeds live; mirror serves Pyth's requested schema and does zero math; every response byte-reproducible | ~6 weeks after Stage 0 |
| **3 — First consumer** | One protocol reads a quote account (or Scope adds an SVI type) in production | Q4 2026 – Q1 2027 |
| **4 — Adapter #2** | A second protocol's feed ships with **zero** changes to `svi-core` / `svi-math` | Q1 2027 |
| **5 — Standardisation** | sRFC submitted, aligned with sRFC 40 rather than competing | Q1–Q2 2027 |

**Stages 3 and 4 are the real test.** Stages 0–2 are within our control and
prove competence. Only stage 3+ proves a market. The kill criteria in the
market analysis (§9) attach here.

---

## 5. Messaging, by audience

The same architecture, argued four different ways. Leading with the wrong one
loses the room.

### To Hylo (partner) — *"you don't have to build or own this"*
The lead is **liability transfer and engineering time**, not technology.
- You were asked to build a price API. That API is a single point of failure you would own, operate and be blamed for.
- SVI removes the arithmetic from it. You keep zero new infrastructure and change zero lines of your program.
- Your own math, called directly from `hylo-core` — SVI reimplements nothing, so SVI's number and your redemption math cannot diverge.
- V2 turns your 4 feeds into 4 × N xAssets. A methodology-per-asset framework scales down that path; a hand-rolled API does not.
- **The ask is five answers and a name**, not a commitment of engineering resource.

### To Pyth (customer) — *"here is exactly the API you asked for"*
The lead is **zero integration cost**.
- Same feeds, same cadence, same response schema you requested from Hylo.
- Difference: it performs no arithmetic. It reads a canonical account through two RPCs, requires byte-for-byte agreement, and serves the decoded value with provenance.
- You can verify any response against the chain yourself — or skip the endpoint entirely and read the account.
- It does not fix publisher key management. That is still yours; we are not going to pretend otherwise.

### To lending markets (consumer) — *"a number you can defend in a post-mortem"*
The lead is **the incident that already happened to your peer**.
- Loopscale, April 2025: $5.8M, 12% of TVL, two weeks after launch — because the *lender's own* internal pricing of a protocol-derived token was wrong.
- Every protocol asset you list today means valuation code you wrote, maintain, and are liable for.
- SVI gives you: a conservative redemption-anchored value, an explicit uncertainty band, a `valid_until_slot`, a `value_type` that cannot silently become a spot price, and a `DESTABILIZED` flag that means *zero*, not *cheap*.
- One account read. Fixed 320-byte layout, compile-time asserted.

### To the Foundation / Colosseum (legitimiser) — *"public-goods infrastructure with a live design partner"*
The lead is **the gap in the current standards stack**.
- sRFC 40 standardises vault NAV flows. It does not cover leveraged tokens, backing NAV, redemption value, or receipt tokens.
- SVI is the semantics layer for *all* protocol-derived value, with a working implementation and a named partner.
- Intent is to contribute into the sRFC process, not to run a parallel standard.
- Open source, no token, no rent-seeking.

---

## 6. Business model

**The standard, the SDKs, the core program and the methodology specs are open
source and free, permanently.** Charging for the interface would kill adoption
and is the one thing that guarantees an incumbent forks it.

Revenue, if it comes, sits beside the standard:

| Line | Model | Gate | Realistic timing |
|---|---|---|---|
| Custom adapter development | $10–40k / protocol | A protocol wants a feed it can't build | Q1 2027+ |
| Certified / reviewed adapters | annual per feed | Post-audit only | Q2 2027+ |
| Managed keeper + mirror | $500–3,000 / mo / protocol | Someone doesn't want to run infra | Q1 2027+ |
| Monitoring & risk dashboards | SaaS | Multiple feeds live | Later |
| Valuation / economic-security reviews | consulting | Reputation established | Opportunistic |

**Honest expectation: low six figures ARR at the very best in year one, and
possibly zero.** Anyone modelling this as venture-scale SaaS is modelling a
different company. The return is position and reputation; the revenue is a
consultancy attached to it. Section 8 covers what actually funds the work.

### Cost base

| Item | Cost |
|---|---|
| Program deployment + account rent | **< $500** one-off |
| Keeper + redundant RPC + monitoring (pilot) | **$50–150 / mo** |
| Broader mainnet pilot | $300–1,200 / mo |
| Audit | **$30–80k**, gated on first real-collateral integration |

The audit is the only large number, and it is deliberately deferred until a
consumer actually depends on a feed. Everything before that point costs less
than a laptop.

---

## 7. Distribution

No paid acquisition; this is a relationship and credibility business.

1. **Warm partner channel.** Hylo is the design partner. A blessed methodology spec and a co-announcement is the single highest-leverage distribution event available.
2. **Consumer-side network.** Direct relationships with lenders (marginfi / P0 network) — exactly the buyers of these feeds. Use them for stage 3, not for stage 1.
3. **The methodology specs are the content marketing.** `hylo-xsol-nav-v1` is a genuinely useful public document. Publishing rigorous per-protocol valuation specs builds the "these people are careful" reputation that oracle infrastructure actually runs on. Nothing else in the plan converts as well.
4. **Colosseum, 28 Sep – 2 Nov 2026.** Public-goods infra with live mainnet feeds and a named partner is exactly the winning shape. Funding path attached (see §8).
5. **The sRFC process.** Slow, compounding, and the highest-ceiling channel.
6. **Incident-driven relevance.** Every protocol-derived mispricing incident is a moment when the argument makes itself. Have the write-up ready.

---

## 8. Funding path

Ranked by fit. Note that none of these require revenue, which is the point.

| Source | Fit | Ask | Notes |
|---|---|---|---|
| **Colosseum** (hackathon → accelerator) | **Excellent** | $250k pre-seed | Next hackathon **28 Sep – 2 Nov 2026**; $2.5M+ deployed to winners at $250k each. Also *already a Hylo investor* — a warm, informed audience |
| **Solana Foundation grants** | **Excellent** | $25–100k | Public-goods infrastructure aligned with an active Foundation standards effort is the archetype |
| **Superteam grants** | Good | $5–25k | Fast, small, good for keeper/audit runway |
| **Pyth / oracle ecosystem grants** | Good | varies | They benefit directly |
| **Audit-specific funding** | Good | $30–80k | Often available separately once a consumer commits |
| **Venture** | **Poor — do not pursue now** | — | The revenue model doesn't support it and taking it would force premature monetisation of a standard |

**Recommended:** Colosseum submission in the 28 Sep – 2 Nov window with Hylo
named as design partner and live mainnet feeds, in parallel with a Foundation
grant application. Stage 0 must close before 28 September for this to work —
which is the reason the ask to Hylo has a date on it.

---

## 9. Milestones

Dates assume Stage 0 closes by **mid-September 2026**.

| When | Milestone | Proof it happened |
|---|---|---|
| **Sept W2** | Hylo answers §10 Q1–Q5; methodology frozen and hashed | Signed spec v1.0, hash published |
| **Sept W3** | Hylo agrees to design-partner naming + co-announce | Public confirmation |
| **Sept W4** | Adapter computes xSOL NAV on devnet; differential test vs `hylo-core` passes | Green CI, published test output |
| **Oct W1** | Observer at zero divergence over 72h; CU < 200k measured | Observer log, CU benchmark |
| **Oct W2** | 4 Hylo feeds live on **mainnet**; keeper running | Quote account addresses published |
| **Oct W3** | Mirror API live in Pyth's requested schema; zero arithmetic | Conformance test suite public |
| **Oct W4** | Pyth evaluating / consuming | Their confirmation |
| **Nov 2** | **Colosseum submission** — live feeds, named partner, spec, observer | Submission |
| **Nov–Dec** | First on-chain consumer commits (Scope type or a lending market) | Their integration PR |
| **Q1 2027** | Adapter #2 ships with zero shared-code changes | Second protocol's feeds live |
| **Q1 2027** | Audit engaged, gated on first real-collateral use | Engagement letter |
| **Q1–Q2 2027** | sRFC submitted, aligned with sRFC 40 | sRFC number |

---

## 10. Metrics

Vanity metrics are excluded deliberately — no GitHub stars, no "feeds
supported" without consumers.

| Category | Metric | Stage-2 target | Stage-4 target |
|---|---|---|---|
| **Correctness** *(the only one that can kill the project)* | Observer divergence events | **0** | **0** |
| Correctness | Quotes published from unvalidated inputs | **0**, always | **0** |
| Liveness | Feed uptime (fresh within `valid_until_slot`) | > 99.5% | > 99.9% |
| Liveness | Median staleness at read time | < 60s | < 30s |
| **Adoption (publish)** | Protocols with live feeds | 1 (Hylo) | 3+ |
| **Adoption (consume)** | Production consumers reading a quote | 1 (Pyth mirror) | 3+ |
| **Adoption (consume)** | On-chain consumers reading the *account* | 0 | 1+ |
| Adoption | TVL valued by SVI feeds | — | first $100M |
| Efficiency | Refresh CU including CPI | < 200k | < 150k |
| Efficiency | Operating cost / month | < $150 | < $1,200 |
| Legitimacy | Named design partners | 1 | 2+ |
| Legitimacy | Audit status | scheduled | complete |

**The metric that matters most is the first one.** A single wrong published
number, once, is unrecoverable for a project whose entire pitch is
verifiability. Correctness is not a KPI here; it is the licence to have KPIs.

---

## 11. Risks and counters

| Risk | Likelihood | Impact | Counter |
|---|---|---|---|
| Pyth waits for Hylo's own API instead | Medium | High — kills the best adoption story | Match their requested schema *exactly* (spec §10 Q3). Make the yes cost nothing. If they still decline, the account is readable directly and lenders remain the target |
| No second protocol adopts | Medium | High — SVI stays a Hylo wrapper | Pick adapter #2 for *consumer* pull (an LST or Meteora feed a lender already wants), not for technical elegance |
| sRFC 40 absorbs the vault slice | Medium | Medium | Don't compete — contribute. Cover what vaults don't: leveraged tokens, backing NAV, redemption, receipt tokens |
| An incumbent ships this in three weeks | Low–Medium | Medium | They've had years and haven't, because it doesn't monetise. Defence is methodology specs, audit, track record, partner relationships — not code |
| **A published value is wrong** | Low | **Fatal** | Depend on `hylo-core` (reimplement nothing); independent observer with zero-tolerance halt; fail-stale-never-fail-wrong; gate real collateral on audit |
| Layout drift after a protocol upgrade | **High** | High | Store target programdata hash in `adapter_config`, verify every refresh, abort on change (user story B4) |
| Liability from a lender's wrongful liquidation | Low | High | Audit before real collateral; explicit machine-readable semantics and disclaimers; documented `DESTABILIZED` guidance |
| Solo-maintainer bus factor | High | Medium | Everything open source; permissionless keeper so anyone can crank; core simple enough to be frozen |
| Hylo says no | Low–Medium | Medium | The architecture requires no permission — the adapter can be built from public account data regardless. What's lost is the blessing, the co-announcement and the five answers. Fall back to a second tier-1 protocol |

**Note on the last row, and it should be said out loud in the meeting:**
SVI can technically build this without Hylo. Asking is a choice, because a
methodology spec the protocol has reviewed is worth more than one it hasn't —
and because building someone's valuation feed without telling them is how you
get an adversarial relationship with the exact partner you need.

---

## 12. Things we will not do

| Never | Because |
|---|---|
| Launch a token | Duplicates Pyth/Switchboard's strongest asset, adds regulatory surface, corrupts a public good |
| Charge for the standard or the SDKs | Guarantees a fork and kills the only real goal — adoption |
| Publish a value from unvalidated inputs | The entire premise |
| Publish an ambiguous feed name like `xSOL/USD` | Semantic ambiguity is failure mode #6 |
| Derive a collateral price from spot pool reserves | Flash-loan manipulable |
| Ship an adapter for a protocol whose math we can't read | Then we'd be guessing, and guessing at scale is the thing we're replacing |
| Take venture money before a second consumer | Forces premature monetisation of a standard |
| Claim SVI removes Pyth's key risk | It doesn't, and overclaiming to a technical audience destroys the credibility the whole pitch runs on |

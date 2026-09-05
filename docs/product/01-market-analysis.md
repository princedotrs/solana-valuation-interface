# SVI — Market Analysis

**Solana Valuation Interface**
Prepared for the Hylo partnership proposal · September 2026

> **How to read this document.** It is written to be argued with. Every market
> number carries a source and a date; every number I could not verify is marked
> as an estimate. Section 8 is a list of reasons *not* to build this, and
> section 9 is the set of conditions under which I stop. If you are Hylo and
> you are deciding whether to spend an engineer-afternoon answering five
> questions, sections 2, 6 and 8 are the ones that matter.

---

## 1. The problem, stated precisely

There are two kinds of assets in DeFi, and the industry prices both with the
same machinery even though only one of them should be.

**Traded assets** — SOL, BTC, USDC, memecoins — have a price because people
trade them. The price is an *observation* of a market. Pyth and Chainlink are
excellent at this, and nothing in this document competes with them.

**Protocol-derived assets** — LST shares, vault receipts, leveraged tokens,
lending receipt tokens, LP positions, principal/yield tokens, tokenized fund
shares — do not have a price in that sense. They have a *value*, defined by a
formula over on-chain state. xSOL is worth `(SOL TVL − vUSD supply) / xSOL
supply` because Hylo's own redemption math says so, whether or not a single
xSOL trades today.

The industry currently prices the second category by pretending it is the
first: someone reads protocol state over RPC, runs private arithmetic in a
server, and publishes the output as if it were an observed market price. The
number arrives on-chain stripped of the one thing that made it trustworthy —
the ability to recompute it.

### 1.1 The exact trust chain, and why it has six failure points

This is the pipeline Pyth asked Hylo to build, drawn honestly:

```
Hylo's on-chain accounts        ← the only real source of truth
        ↓  (1) one RPC provider
        ↓  (2) a custom off-chain parser
        ↓  (3) private calculation code
        ↓  (4) a Hylo-operated API
        ↓  (5) a Pyth publisher process + key
        ↓  (6) on-chain price account
consumed as collateral value by a lender
```

Each arrow is a place the number can change without anyone noticing:

| # | Failure mode | Why it is not theoretical |
|---|---|---|
| 1 | **RPC inconsistency / manipulation** — a single provider serves a stale or forked view | Standard practice is one provider; nothing in the pipeline detects disagreement |
| 2 | **Parser drift** — the protocol upgrades an account layout, the parser silently misreads it | The single most common cause of quiet mispricing in adapter-based systems |
| 3 | **Hidden methodology** — nobody downstream can reproduce the arithmetic | The consumer cannot tell a bug from a market move |
| 4 | **API compromise** — server, DNS, dependency, or insider | An API that computes a number *is* a price oracle with one operator |
| 5 | **Signer compromise** — a publisher key can sign any number | The number's provenance ends at "a key said so" |
| 6 | **Semantic ambiguity** — the field says `xSOL/USD` and does not say whether that is NAV, redemption value, or last trade | A lender that wanted NAV and got market spot has silently taken a different risk |

Plish's original message identified this correctly and without prompting:

> *"honestly an accepted smart contract interface for pricing onchain assets
> would be amazing — bc pyth wants us to read our own protocol by RPC and give
> them data by an API — it leaves all sorts of unimaginable security holes"*
> — ◎ Plish | Hylo, 28 Aug 2026

The point of SVI is not that RPC is forbidden. Every off-chain service reads
the chain somehow. The point is **where the arithmetic happens**. If the math
runs on-chain and the result is stored in a public account, then RPC, APIs and
publisher keys are demoted from *authors* of the number to *couriers* of it —
and a courier can be checked against the original by anyone.

---

## 2. Is the pain real, and does anyone pay for it?

Four independent lines of evidence, strongest first.

### 2.1 A named protocol asked for this, unprompted

Hylo is being pushed into building the exact unsafe pipeline above, by Pyth,
right now. That is not a survey response — it is a live requirement with a
counterparty and a deadline. This is the single most important fact in this
analysis, and it is also the weakest kind of evidence for a *market*: it is
n=1.

### 2.2 Protocol-derived mispricing has a body count

These are not flash-loan attacks on thin AMM pools. Each one is a case where a
*protocol-derived* value — an exchange rate, a receipt-token price, a share
price — was computed or configured wrongly, and a lender ate it:

| Incident | Date | Loss | Root cause |
|---|---|---|---|
| **Loopscale** (Solana) | Apr 2025 | **$5.8M** (~12% of TVL, 2 weeks after launch) | The lender's own internal pricing of RateX principal tokens was wrong; attacker took undercollateralized loans against inflated PT value ([Halborn](https://www.halborn.com/blog/post/explained-the-loopscale-hack-april-2025), [The Block](https://www.theblock.co/post/352083/solana-defi-protocol-loopscale-hit-with-5-8-million-exploit-two-weeks-after-launch)) |
| **Moonwell** | 2025 | **$1.8M bad debt** | An oracle *config change* priced cbETH via cbETH/ETH only, omitting the ETH/USD leg — cbETH read $1.12 instead of ~$2,200; liquidation bots did the rest |
| **Mountain wUSDM / Venus** | Feb 2026 | self-liquidation exploit | ERC-4626 donation attack pushed the share exchange rate 1.06 → 1.7; the *rate itself* was the attack surface |
| **YieldBlox** | Feb 2026 | **$10.2M** | Thin-liquidity VWAP oracle on a tokenized-treasury pair; inflated collateral value |

Context: Q1 2026 alone saw **$137M+** lost across DeFi exploits, and 2026 has
passed **$1B+** cumulatively ([Q1 pattern analysis](https://dev.to/ohmygod/q1-2026-defi-exploit-pattern-analysis-137m-lost-5-attack-patterns-every-auditor-must-know-2mh),
[CCN](https://www.ccn.com/education/crypto/defi-hacks-exploits-causes-crypto-stolen-2026/)).

Note the pattern in all four: **no on-chain account was ever wrong.** Hylo's
accounts, Coinbase's cbETH contract, Mountain's vault — all correct. The loss
came entirely from the *valuation layer* between the true state and the
consumer. That is precisely the layer SVI replaces with reproducible code.

The Loopscale case is worth dwelling on, because Loopscale currently lists
hyUSD. A lender that has to hand-roll valuation for every protocol asset it
accepts will eventually hand-roll one wrong.

### 2.3 Everyone has independently built a private version of this

The strongest sign a standard is missing is that every large player has built
their own:

- **Kamino Scope** maintains roughly 40 oracle "types" — one per asset shape —
  because there is no shared interface. It is an in-house SVI with no external
  surface ([repo](https://github.com/Kamino-Finance/scope), [docs](https://kamino.com/docs/risk/asset-risk/oracle-pricing)).
- **Switchboard** lets you write custom TEE jobs that parse protocol accounts —
  same problem, solved off-chain per-integrator.
- **Ethereum** shipped **ERC-4626** for vault share conversion and has
  **ERC-7726** proposed for a common quote interface. Solana has no equivalent.
- **Solana Foundation** is actively pursuing **sRFC 40**, a canonical vault
  standard with async deposit/redemption settled *once NAV is updated* —
  built with Exo Technologies, paired with sRFC 37 (Token ACL)
  ([solana-foundation/vault](https://github.com/solana-foundation/vault),
  [Exo](https://exotechnologies.xyz/research/solana-needs-something-like-erc4626-vaults)).

Four independent teams converging on the same missing primitive is the market
telling you it exists. It is *also* telling you the incumbents can build it,
which is section 8.

### 2.4 The asset class is growing fast, and it is the wrong asset class for market-price oracles

| Segment (Solana) | Size | Date | Source |
|---|---|---|---|
| Non-stablecoin tokenized RWA | **$3.7B**, ~313K holders | Jul 2026 | [Blockchain.News](https://blockchain.news/news/solana-rwa-ecosystem-growth-2026) |
| — same, six months earlier | $873M | Jan 2026 | [The Crypto Basic](https://thecryptobasic.com/2026/07/10/solana-tokenized-rwa-market-soars-4x-hits-record-3-62b-in-h1-2026/) |
| LST supply | approaching **$15B** | May 2026 | [Sanctum](https://sanctum.so/blog/solana-liquid-staking-yields-ranked-highest-paying-lsts-2026) |
| Stablecoin supply | ~$16B | 2026 | [CryptoBriefing](https://cryptobriefing.com/solana-rwa-value-stablecoin-supply-institutional/) |
| Lending TVL (Kamino / marginfi / Save / Drift spot) | ~$2.1B / ~$700M / ~$400M / ~$300M | 2026 | [Eco](https://eco.com/support/en/articles/14801186-kamino-lending-solana-s-money-market-explained) |

Solana's RWA market **quadrupled in six months** and is the third-largest
tokenized-asset chain at ~10.4% of a ~$27.65B total. Tokenized funds, treasury
products and vault shares are, definitionally, NAV-priced instruments — not
market-priced ones. Every one of them arriving on Solana is a unit of demand
for exactly this primitive, and Pyth's 2,800+ feeds and 128 publishers are
built for the other category.

---

## 3. Market sizing — honest version

I am going to refuse to give you a $10B TAM slide. Here is the real shape.

**TAM (value that must be correctly valued).** The collateral base that
depends on protocol-derived valuation on Solana today: ~$15B LST supply +
$3.7B RWA + several billion in vault/LP/receipt tokens. Call it **$20B+ of
assets whose value is a formula, not a trade.** This is the number that
matters for *impact*, and it is genuinely large.

**SAM (what SVI could plausibly serve).** Feeds consumed by lending markets,
oracles and structured products for protocol-derived assets. Bounded by the
number of (protocol × asset) pairs: today on Solana, order **300–800 distinct
feeds** across LSTs, vault shares, leveraged tokens, LP positions and RWA
shares. Growing with the asset class.

**SOM (revenue, realistically).** This is where I break with the optimistic
version of this pitch. **Standards do not monetize.** The revenue-bearing
slice is services around the standard:

| Line | Model | Realistic year-1 |
|---|---|---|
| Custom adapter development | $10–40k per protocol adapter | 2–5 adapters |
| Certified / independently-reviewed adapters | annual fee per feed | 0 until audited |
| Managed keeper + mirror infrastructure | $500–3,000/mo per protocol | 1–3 customers |
| Monitoring & risk dashboards | SaaS | later |
| Economic-security / valuation reviews | consulting | opportunistic |

**Year-1 realistic revenue: low six figures, at best, and only after
oracle-grade trust is earned.** Anyone modelling this as a venture-scale
standalone SaaS is modelling a different company.

The honest framing: **SVI's first-order return is not revenue. It is
position.** Specifically — being the author of the valuation interface that
Solana adopts, with production feeds and named design partners, at exactly the
moment the Foundation is standardising the adjacent layer (sRFC 40). That
position is worth more than the ARR, and it is winnable by one person in a
quarter, which almost nothing else in infrastructure is.

**Cost to find out** is the reason to proceed regardless: program deployment
is **< $500 in SOL**, keeper + redundant RPC + monitoring runs **$50–150/month**
for a single-protocol pilot (rising to ~$300–1,200/mo for a broader mainnet
pilot), and the only large cost — a **$30–80k audit** — is not incurred until
real collateral actually depends on it. The asymmetry is extreme: a few weeks
of engineering against a shot at authoring a Solana standard.

---

## 4. Competitive landscape

| | What it is | Overlap with SVI | Why SVI is not redundant |
|---|---|---|---|
| **Pyth** | Push/pull oracle, 2,800+ feeds, 128 publishers, institutional | **Customer, not competitor.** Pyth is asking Hylo for this data today | Pyth is built for *observed market prices*. It has no mechanism for reproducible protocol-derived NAV; it is currently outsourcing that arithmetic to protocols' own servers |
| **Switchboard** | Permissionless custom feeds, TEE-executed jobs | Real overlap: a Switchboard job can parse Hylo accounts today | The math still runs off-chain in an enclave. You trade "trust the API" for "trust the enclave + job definition". SVI's computation is a Solana transaction anyone can replay |
| **Kamino Scope** | Oracle aggregator, ~40 asset-specific valuation types | Closest operational analogue — Scope *is* the adapter model, proven | Scope is Kamino's internal component, curated by Kamino, consumed by Kamino. It is not an interface other protocols publish through, and Kamino has no incentive to make it one. Scope is a **consumer target**, not a rival |
| **Blueshift Doppler** | Ultra-efficient price publishing primitive (~21 CU reads) | None on semantics | Doppler is *transport*. It makes writing a number cheap; it says nothing about where the number came from. SVI could publish through Doppler-style rails |
| **sRFC 40 vault standard** | Foundation + Exo canonical vault standard with NAV-settled flows | **Genuine strategic risk** | If `get_nav` becomes canonical, the vault-share slice is absorbed. Mitigation is not to compete: contribute SVI's semantics *into* sRFC and cover what vaults don't — leveraged tokens, LP, receipt tokens, backing NAV, redemption value |
| **Chainlink NAV oracles** | Bringing off-chain fund NAV on-chain for RWAs | Adjacent, opposite direction | Chainlink transports NAV *computed off-chain by a fund administrator*. SVI computes NAV *from on-chain state*. Different trust model, different asset set; they meet at tokenized funds |

**The honest competitive read:** there is no one doing exactly this, and that
is *not* because it is technically hard. It is because it is not obviously
profitable. Kamino, Switchboard and Blueshift could each build it in weeks if
they wanted to. The defensible assets are therefore not code:

1. **Methodology specs** — versioned, hashed, co-signed by the protocol whose
   math they encode. Hylo blessing `hylo-xsol-nav-v1` is worth more than the
   adapter binary.
2. **Audit + track record** — the only real moat in oracle infrastructure.
3. **Being first into the sRFC process** with a working implementation.
4. **Consumer-side relationships** — the hard side of the market (§6.2).

---

## 5. Why now

Five things are true simultaneously, and were not 12 months ago:

1. **A design partner has the problem today, with a deadline.** Pyth has asked
   Hylo for the API. That request expires — either Hylo builds the unsafe
   version or something better exists first.
2. **Hylo V2 multiplies the problem.** V2 (beta live late March 2026) shipped
   the **xAsset Engine** — multi-asset collateral and leveraged products over
   crypto, **equities and commodities**, with the Stability Pool replaced by an
   Earn Pool ([KuCoin](https://www.kucoin.com/news/flash/hylo-launches-v2-expands-on-chain-leverage-to-stocks-and-multi-asset-collateral)).
   Hylo goes from ~4 feeds to *4 feeds per xAsset*. Hand-rolled APIs do not
   scale down that path; a methodology-per-asset framework does.
3. **The RWA wave is NAV-shaped and it is arriving now.** 4x growth in six
   months on Solana, and every tokenized fund needs a value, not a price.
4. **The Foundation is standardising the neighbouring layer.** sRFC 40 is in
   flight. Contributing into an open process beats maintaining a parallel one,
   and the window to be early in that process is open now.
5. **The next Colosseum hackathon runs 28 Sep – 2 Nov 2026.** Public-goods
   infrastructure with a named design partner and live mainnet feeds is
   precisely the shape that wins there, and it comes with a real funding path
   (Colosseum deployed $2.5M+ into winners at $250k each)
   ([Colosseum](https://blog.colosseum.com/announcing-colosseum-eternal-and-solanas-2025-hackathon-schedule/)).

---

## 6. Where the difficulty actually is

### 6.1 Not the publisher side — that problem is already solved

The original framing of this idea required protocols to implement an interface
in their own programs. That framing is dead, and killing it is the most
important design decision in the project.

Solana programs have no view functions. But they do not need them: **account
data is public bytes**. An adapter takes the target protocol's accounts as
transaction inputs, verifies owner / PDA derivation / mint / program ID, and
deserializes them directly. All reads inside one transaction are same-slot
consistent by construction — no CPI, no CPI-depth budget, no cooperation.

Consequence: **SVI never has to ask Hylo, Meteora, Raydium or Sanctum to
change a line of code.** It is the Kamino Scope model, and Scope proves it
works in production at billions of dollars of TVL.

CPI is needed in exactly one case: state that must be *accrued* before it is
correct — lending receipt tokens that lazily accrue interest, or Hylo's
`TotalSolCache` at an epoch boundary. There, the refresh transaction calls the
protocol's own permissionless crank first, then reads. Still no cooperation
required. (Hylo's `update_lst_prices` is payer-signer-only — permissionless —
which is why §5.1 of the methodology spec works.)

### 6.2 Yes, the consumer side — this is the real problem

Eliminating publisher-side cooperation moves 100% of the difficulty to
adoption. SVI must convince **Pyth** to consume the mirror, **Scope or a
lending market** to read the quote account, and eventually a **second
protocol** to publish through the interface. That is a partnerships problem,
not an engineering one, and it is the part that can actually fail.

Two assets help here and should be used deliberately: the existing Hylo
relationship, and a consumer-side network (marginfi / P0) of exactly the kind
of lender that would read these feeds.

### 6.3 The operational risk that will bite first: layout drift

When a target protocol upgrades, an adapter can silently misprice. This is the
Moonwell failure and it is the most likely way SVI hurts someone.

Mitigation, designed into v1 rather than bolted on: store the target program's
deployed-slot / programdata hash in the adapter config, check it on **every**
refresh, and if it changed, refuse to publish and let the quote age out.
Combined with the independent observer (§8 of the methodology spec) this moves
drift from *silent* to *loud*. The design rule stated once:

> **Fail stale, never fail wrong.**

### 6.4 Things SVI must refuse to do

- **Never derive collateral prices from spot pool reserves.** Dividing reserves
  is flash-loan manipulable. Where a market price is genuinely needed, read the
  protocol's own TWAP/observation accounts and tag the result `MARKET_SPOT` /
  `MARKET_TWAP` with explicit flags so no lender can mistake it for NAV.
- **Never launch a token, an operator network, or slashing.** That duplicates
  the strongest part of Pyth/Switchboard while multiplying complexity.
- **Never publish an ambiguous feed named `xSOL/USD`.** Value type is a
  first-class field, enforced on-chain (`ValueType` discriminant, checked in
  `initialize_feed`).

---

## 7. Liability — the thing nobody puts in a market analysis

The moment a lending market uses an SVI quote to value real collateral, a
mispricing bug produces wrongful liquidations, and that lands on SVI
reputationally and plausibly legally.

Practical consequences, which are also product requirements:

- Audit stops being optional at the point of first collateral integration, not
  before. Budget $30–80k and gate mainnet-with-real-collateral on it.
- Explicit, machine-readable disclaimers: `value_type`, `status_flags`,
  `valid_until_slot` and `methodology_hash` exist so a consumer can never claim
  it did not know what it was reading.
- The independent observer with mirror-halt-on-divergence is a liability
  control as much as a technical one.

---

## 8. The case against building this

Stated at full strength, because a proposal that cannot survive its own
counter-argument should not be presented to Hylo.

1. **It does not monetize well.** Standards are public goods. The services
   layer is a consultancy. (§3)
2. **Incumbents can absorb it in weeks.** Scope already does adapter-per-asset
   internally. None of them has shipped the external version because the
   revenue isn't there — not because they can't. (§4)
3. **sRFC 40 may standardise the best slice out from under it.** If Foundation
   `get_nav` hooks become canonical, vault shares — the largest clean segment —
   get absorbed. (§4)
4. **The adoption side is entirely outside SVI's control.** Pyth may simply
   wait for Hylo's own API. (§6.2)
5. **It might end up as a Hylo-only wrapper**, in which case it is a favour to
   one protocol, not a standard.
6. **SVI becomes the trusted party.** Consumers end up trusting SVI's
   reimplementation of someone else's math. That is the business, and it is a
   real, permanent burden. (The mitigation — depending on `hylo-core` directly
   and reimplementing nothing — is the single best design decision in the spec,
   and it does not generalise to every protocol.)

**Verdict: build it — as a narrow product, a reputation play, and a standards
position. Not as a startup expecting revenue.** The asymmetry in §3 (a few
weeks and <$500 against authoring a Solana standard) survives every objection
above. The objections are reasons to keep the scope narrow and the honesty
high, not reasons to stop.

---

## 9. Kill criteria

Written in advance so they cannot be rationalised away later.

**Stop generalising** (keep the Hylo-specific adapter, abandon the standard) if:

- Pyth refuses both direct account verification *and* a pure-mirror API.
- No protocol other than Hylo wants to publish through the interface.
- No downstream protocol (Scope, a lending market, Switchboard) wants to
  consume a quote.
- The values people actually want turn out to be admin-attested rather than
  deterministically derivable from chain state.

**Even in the kill case, the Hylo adapter is still worth having shipped**: it
removes the security exposure of the pipeline Hylo is being asked to build, at
a cost of a few weeks. That is the floor on this project's value, and the floor
is above zero.

## 10. Success criteria for the pilot

1. Hylo's xSOL, hyUSD and eHYUSD values publish through deterministic on-chain
   adapters.
2. The mirror API performs **zero** financial arithmetic.
3. Every API response is reproducible byte-for-byte from a canonical Solana
   account.
4. Pyth consumes the bridge without trusting hidden Hylo backend math.
5. An independent observer reaches exact parity with the on-chain computation.
6. An auditor finds no human-controlled writer anywhere in the deterministic
   valuation path.
7. A second protocol implements an adapter.
8. Scope, Switchboard or a lending protocol consumes at least one SVI quote.

Items 1–6 are within SVI's control and are the pilot. Items 7–8 decide whether
this becomes a standard, and they are the real test.

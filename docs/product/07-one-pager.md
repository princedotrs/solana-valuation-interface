# SVI — One Pager

### Solana Valuation Interface
*On-chain valuation for assets whose value is a formula, not a trade.*

---

**THE PROBLEM.** Protocol-derived assets — leveraged tokens, vault shares,
LSTs, receipt tokens, tokenized funds — don't have a price set by trading. They
have a *value* defined by a formula over on-chain state. The industry prices
them anyway by pretending otherwise: a server reads protocol state over RPC,
runs private arithmetic, and publishes the output as if it were an observed
market price. The number arrives having lost the one property that made it
trustworthy — the ability for anyone to recompute it.

That pipeline has **five trusted hops** and no way for a consumer to check any
of them. Hylo is being asked by Pyth to build exactly this, today.

---

**WHAT BREAKS.** The valuation layer, not the chain state:

| Loopscale · Apr 2025 | **$5.8M** — the lender's own pricing of RateX PT tokens was wrong |
| Moonwell · 2025 | **$1.8M bad debt** — a config change dropped the ETH/USD leg from cbETH; bots liquidated |
| wUSDM / Venus · Feb 2026 | share exchange rate pushed 1.06 → 1.7 by donation attack |
| YieldBlox · Feb 2026 | **$10.2M** — thin-liquidity VWAP on a tokenized treasury |

In every case **no on-chain account was ever wrong.**

---

**THE SOLUTION.** The value is computed by a program *on-chain*, from accounts
it verifies (owner, PDA derivation, mint, program ID), and written to a public
320-byte account anyone can read and independently reproduce byte-for-byte.

- **Adapter** — one per protocol. Reads the protocol's public account bytes and calls the protocol's own math library. Reimplements nothing.
- **Core** — tiny, protocol-agnostic, auditable, freezable. Only a registered adapter's PDA may write. Sequence monotonic, freshness enforced.
- **Quote account** — the product. Value + bounds + `observed_slot` + `valid_until_slot` + `methodology_hash` + `value_type`.
- **Keeper** — permissionless. Pays gas; **cannot influence the value**.
- **Observer** — independently recomputes and halts the mirror on any divergence.

> **Fail stale, never fail wrong.** Any unverified input aborts the refresh.
> No code path publishes a value computed from unvalidated inputs.

**Zero cooperation required.** Solana programs have no view functions and none
are needed — account data is public bytes, and all reads inside one transaction
are same-slot consistent. No protocol ever changes a line of code.

---

**MARKET.** ~$20B+ on Solana whose value is a formula: ~$15B LST supply,
**$3.7B** tokenized RWA (**4× in six months**), plus vault, LP and receipt
tokens. Lending markets carrying the valuation risk today: Kamino ~$2.1B,
marginfi ~$700M, Save ~$400M, Drift spot ~$300M.

**Competitive position.** Pyth and Chainlink price *traded* assets — SVI has
nothing to say about SOL/USD and Pyth is customer #1. Kamino Scope proves the
adapter model at scale but is internal to Kamino. Switchboard runs the math
off-chain in a TEE. sRFC 40 standardises vault NAV flows only. **Nobody
publishes reproducible on-chain valuation as an open interface.**

---

**STATUS.** `svi-math` ✅ done · `svi-core` ✅ done · `svi-mock-adapter` ✅ done ·
`hylo-xsol-nav-v1` methodology spec 🟡 draft v0.9 · `svi-hylo-adapter` 🔒
blocked on five questions to Hylo. Keeper, mirror API and observer are phase 2.

**Cost to reach mainnet:** <$500 deployment, $50–150/mo operating. Audit
($30–80k) deferred until a lender actually depends on a feed.

---

**THE ASK — HYLO.** Five technical answers (one engineer-afternoon), permission
to name Hylo as design partner, and a co-announcement at launch. **No program
changes, no engineering sprint, no funding, no exclusivity.**

**TIMELINE.** ~6 weeks from spec freeze to Pyth evaluating. Colosseum runs
**28 Sep – 2 Nov 2026**.

---

**HONEST CAVEATS.** Standards don't monetise — realistic year-one revenue is low
six figures at best, from services beside a permanently free standard. This is a
position and reputation play, not a venture-scale SaaS. Incumbents could build
it in weeks; the defensible assets are methodology specs, audits, track record
and partner relationships, not code. SVI does **not** remove Pyth's publisher
key risk, and claiming otherwise would be dishonest.

*github.com/princedotrs/solana-valuation-interface*

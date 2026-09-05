# SVI — Product Documentation

**Solana Valuation Interface** · prepared for the Hylo partnership proposal
September 2026

> **The one-sentence version:** the value of a protocol asset is computed by a
> program on-chain, from accounts anyone can verify, and written to a public
> account anyone can read — so nobody ever has to trust an off-chain API doing
> hidden math.

---

## Reading order

| # | Document | What it answers | Read time |
|---|---|---|---|
| — | [**One pager**](07-one-pager.md) | Everything, compressed to a page. The leave-behind | 2 min |
| 01 | [**Market analysis**](01-market-analysis.md) | Is this worth building? Who else is doing it? What kills it? | 15 min |
| 02 | [**Product overview**](02-product-overview.md) | What is SVI, in plain language, with one running example | 10 min |
| 03 | [**Architecture**](03-architecture.md) | How it works — diagrams, data model, failure states, design rationale | 15 min |
| 04 | [**User stories**](04-user-stories.md) | What gets built, in what order, with acceptance criteria and the pre-mainnet gate | 15 min |
| 05 | [**Go-to-market**](05-gtm.md) | Positioning, wedge sequence, business model, funding, milestones, metrics | 15 min |
| 06 | [**Hylo proposal**](06-hylo-proposal.md) | The ask, what Hylo gets, the five questions, pilot plan, objections | 10 min |
| — | [**Methodology spec**](../hylo-xsol-nav-v1-spec.md) | The exact accounts, formulas, rounding and edge cases for `hylo-xsol-nav-v1` | 15 min |

**If you have five minutes:** the one pager, then §6 of the Hylo proposal (the
five questions).

**If you're Plish:** the Hylo proposal, then §§2–6 of the methodology spec —
that's the fastest way to check whether SVI has understood Hylo's math
correctly.

---

## Presentation materials

| Asset | Path |
|---|---|
| Pitch deck (PowerPoint) | [`../deck/svi-hylo-partnership-deck.pptx`](../deck/) |
| Architecture diagram (SVG + PNG) | [`../diagrams/svi-architecture.svg`](../diagrams/svi-architecture.svg) |
| Trust-chain before/after (SVG + PNG) | [`../diagrams/svi-trust-chain.svg`](../diagrams/svi-trust-chain.svg) |
| Diagram sources | [`../diagrams/gen_architecture.py`](../diagrams/gen_architecture.py), [`gen_trustchain.py`](../diagrams/gen_trustchain.py) |

Diagrams are generated — edit the `.py` and re-run to regenerate both SVG and
`@2x` PNG.

---

## Build status

| Component | State | Path |
|---|---|---|
| Exact integer math library | ✅ Done | [`crates/svi-math`](../../crates/svi-math) |
| Core program (quote accounts, authorization) | ✅ Done | [`programs/svi-core`](../../programs/svi-core) |
| Mock adapter (proves the CPI path) | ✅ Done | [`programs/svi-mock-adapter`](../../programs/svi-mock-adapter) |
| `hylo-xsol-nav-v1` methodology | 🟡 Draft v0.9 | [`docs/hylo-xsol-nav-v1-spec.md`](../hylo-xsol-nav-v1-spec.md) |
| Hylo adapter | 🔒 Scaffolded, blocked | [`programs/svi-hylo-adapter`](../../programs/svi-hylo-adapter) |
| Keeper · mirror API · observer | ⬜ Phase 2 | — |

**The critical path is five answers from Hylo** (methodology spec §10), not
engineering capacity.

---

## The design rule everything follows

> ### Fail stale, never fail wrong.

No code path may publish a value computed from unvalidated inputs. A degraded,
visibly-expired quote is always preferable to a plausible wrong one.

---

## Source material

Background research and the original conversations that led here are in
[`../context/`](../context/) — the Telegram thread with Plish, and the ChatGPT
and Gemini research transcripts.

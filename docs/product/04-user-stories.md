# SVI — User Stories

**Solana Valuation Interface** · v1 backlog · September 2026

Status legend: **✅ Done** · **🟡 Partial** · **⬜ Not started** · **🔒 Blocked on Hylo answers** (spec §10)

---

## Personas

| # | Persona | Who they are | What they actually want |
|---|---|---|---|
| **P1** | **Protocol engineer (Hylo)** | Plish and team. Ships the protocol; is being asked by Pyth to build a pricing API | To *not* build and operate a price API, and to not be blamed when one misprices |
| **P2** | **Oracle integration engineer (Pyth)** | Needs xSOL/hyUSD values in Pyth's publisher format | A feed they can ingest today, without auditing someone's backend |
| **P3** | **Risk lead (lending market)** | Kamino, marginfi, Loopscale. Decides what can be collateral | A conservative, unambiguous, freshness-stamped value — and to know instantly when it is not trustworthy |
| **P4** | **On-chain integrator** | Writes a Solana program that reads a value | One cheap account read, a stable layout, and an unmistakable "this is stale" signal |
| **P5** | **Keeper operator** | Anyone with a bot and some SOL | To crank a feed without permission, and to not be a trusted party |
| **P6** | **SVI operator** | Prince / SVI | To ship adapters fast, to sleep at night, and to be able to prove correctness |
| **P7** | **Auditor / security reviewer** | OtterSec, Neodyme, MadShield, or a lender's internal review | To find every writer in the valuation path, and to reproduce every published number |
| **P8** | **xSOL holder** | End user, indirect beneficiary | To not be wrongly liquidated because someone's server did bad arithmetic |

---

## Epic A — Publish a trustworthy value

### A1 · Verify every input account before using it 🔒
> **As** the SVI operator (P6), **I want** the adapter to verify owner, PDA derivation, mint address and program ID for every account **so that** nobody can pass in a forged pool account stuffed with fake tokens.

**Acceptance criteria**
- Given an account whose `owner` is not the Hylo Exchange program, when refresh runs, then the transaction aborts and **no write occurs**.
- Given an account at a pubkey that does not re-derive from its documented PDA seeds, then abort.
- Given an xSOL mint other than `4sWNB8zGWHkh6UnmwiEtzNxL4XrN7uK9tosbESbJFfVs`, then abort.
- Given a Pyth account not owned by the Pyth program or with the wrong feed ID, then abort.
- Every abort path is covered by a negative test that asserts the quote account is byte-identical before and after.

*Spec §3. Blocked on §10 Q1 (V1 vs V2 account layout).*

---

### A2 · Compute NAV with the protocol's own math, not a reimplementation 🔒
> **As** a protocol engineer at Hylo (P1), **I want** SVI to call `hylo-core` directly rather than reimplement my equations **so that** SVI's number and my redemption math can never disagree.

**Acceptance criteria**
- The adapter depends on `hylo-core` pinned to an exact git SHA, recorded on-chain in `adapter_config`.
- No Hylo formula is reimplemented anywhere in the SVI codebase.
- `nav_redeem` comes from `exchange_math::next_levercoin_redeem_nav`; `nav_mint` from `next_levercoin_mint_nav`; CR from `exchange_math::collateral_ratio`.
- A test asserts SVI's published value equals a direct `hylo-core` call on the same inputs, for ≥1,000 randomized states.

*Spec §2. Blocked on §10 Q4 (will Hylo tag SDK releases against deployed programs?).*

---

### A3 · Never lose money to a rounding direction ✅
> **As** a risk lead (P3), **I want** rounding to always fall against whoever benefits **so that** I am never told collateral is worth more than it is.

**Acceptance criteria**
- ✅ Every division takes an explicit `Rounding` argument — there is no default. (`svi-math::mul_div`)
- ✅ Intermediate products are computed in `u128`; a `u64`-overflowing result returns `None`, never wraps.
- ✅ No floating point anywhere: `#![cfg_attr(not(test), no_std)]`, `#![deny(clippy::arithmetic_side_effects)]`.
- ✅ Every function is total — returns `Option`, never panics.
- ✅ `quote_amount` / `lower_quote_amount` round down; `upper_quote_amount` rounds up.
- ✅ Property tests assert `mul_div_floor(a,b,d) <= mul_div_ceil(a,b,d)` and that ceil exceeds floor by at most 1.

*Implemented: `crates/svi-math/src/lib.rs`, `crates/svi-math/tests/properties.rs`.*

---

### A4 · Publish uncertainty, not a false point estimate ✅ (core) / 🔒 (adapter)
> **As** a risk lead (P3), **I want** every value to arrive with a lower and upper bound derived from real oracle confidence **so that** I can size haircuts on evidence instead of guesswork.

**Acceptance criteria**
- ✅ Core rejects any update where `lower > value` or `value > upper` (`SviError::InvalidBounds`).
- 🔒 `lower = nav_redeem` (floor math, Pyth `price.lower`); `upper = nav_mint` (ceil math, `price.upper`).
- 🔒 `quote_amount = nav_redeem` — the redemption-anchored, conservative side.
- ✅ `Band::within_tolerance(max_bps)` exists so a quote too uncertain to be useful can be rejected rather than published.

*Spec §6. Core implemented in `svi-core/src/lib.rs` Rule 3.*

---

### A5 · Only registered adapter code may write a value ✅
> **As** an auditor (P7), **I want** exactly one writer per feed, and for that writer to be a program-controlled PDA **so that** no human wallet can ever submit an arbitrary xSOL price.

**Acceptance criteria**
- ✅ `publish_quote` requires `adapter_authority` to be a `Signer` and to equal `descriptor.adapter_authority`, else `UnauthorizedAdapter`.
- ✅ Because only the adapter program can sign for its own PDA, provenance is pinned to specific code, not to a key a person holds.
- ✅ A test proves a plain wallet signer cannot publish, even with otherwise valid data.
- ✅ A test proves a *different* registered adapter cannot write to this feed.

*Implemented: `svi-core/src/lib.rs` Rule 2; exercised by `programs/svi-mock-adapter`.*

---

## Epic B — Fail stale, never fail wrong

### B1 · Refuse to publish rather than publish something wrong ✅ (core) / 🔒 (adapter)
> **As** a risk lead (P3), **I want** the feed to go stale rather than guess **so that** a degraded input can never masquerade as a healthy price.

**Acceptance criteria**
- ✅ `observed_slot > clock.slot` → `ObservationFromFuture`, no write.
- ✅ `clock.slot - observed_slot > max_age_slots` → `ObservationTooOld`, no write.
- ✅ `observed_slot <= stored.observed_slot` → `StaleObservation`, no write (monotonicity).
- 🔒 Pyth stale, wrong feed ID, or confidence outside **Hylo's own** `OracleConfig` tolerance → abort, no write.
- 🔒 Target program's deployed programdata hash differs from `adapter_config` → abort, no write.

*Spec §5.3. Core implemented: `svi-core/src/lib.rs` Rule 4.*

---

### B2 · Survive the epoch boundary without lying 🔒
> **As** the SVI operator (P6), **I want** the refresh to bundle Hylo's own permissionless LST crank when the cache is a epoch behind **so that** a new epoch does not silently produce a stale NAV.

**Acceptance criteria**
- Given `TotalSolCache.current_update_epoch != clock.epoch`, when refresh runs, then the adapter CPIs Exchange `update_lst_prices` (payer-only signer) in the same transaction, re-reads, and proceeds.
- The published quote sets `EPOCH_BOUNDARY_CPI`.
- Given the CPI cannot be included (compute budget) or fails, then abort with no write; the existing quote ages out via `valid_until_slot`.
- A devnet test crosses a real epoch boundary and asserts both paths.

*Spec §5.1. Blocked on §10 Q2 (any constraint preventing `update_lst_prices` via third-party CPI?).*

---

### B3 · Say "zero", loudly, when the protocol is destabilized 🔒
> **As** a lending market (P3), **I want** an unmissable signal when Hylo's CR falls under 100% **so that** my liquidation engine treats xSOL as worthless rather than as a dip to buy.

**Acceptance criteria**
- Given CR < 100%, then publish `quote_amount = lower = upper = 0` with `DESTABILIZED | OPERATIONS_HALTED`.
- The companion `hylo-hyusd-backing-v1` feed is refreshed in the same window using `depeg_stablecoin_nav`, so hyUSD consumers see true backing < $1.
- Integration guidance states explicitly: `DESTABILIZED` means collateral value **zero**.
- Given `xsol_supply == 0`, then NAV = exactly `1.000000000` on both bounds with `ZERO_SUPPLY_DEFAULT` — matching Hylo's documented behaviour, encoded and tested rather than left as an undocumented API convention.

*Spec §5.2, §4 edge cases.*

---

### B4 · Make layout drift loud instead of silent ⬜
> **As** an SVI operator (P6), **I want** the adapter to detect that a target program was upgraded **so that** I never silently misprice against a changed account layout.

**Acceptance criteria**
- `adapter_config` stores the target program's deployed slot / programdata hash.
- Every refresh compares it; on mismatch the refresh aborts and an operator alert fires.
- The quote ages out naturally; consumers see the expiry.
- Documented as the mitigation for the Moonwell-class failure (a config/upgrade change silently changing what a feed means).

*Market analysis §6.3. This is the highest-probability real-world failure and it has no code yet.*

---

## Epic C — Consume a value safely

### C1 · Read a value in a few hundred compute units ✅
> **As** an on-chain integrator (P4), **I want** one fixed-layout account read **so that** I can value collateral inside a liquidation transaction without a compute-budget fight.

**Acceptance criteria**
- ✅ `Quote` is `#[account(zero_copy)] #[repr(C)]`, exactly **320 bytes**, 8-byte aligned.
- ✅ A compile-time assertion fails the build if the layout ever drifts: `assert!(size_of::<Quote>() == 320)`.
- ⬜ A published offsets table and a `no_std` reader crate so a consumer need not depend on Anchor.
- ⬜ A measured CU benchmark for the read path.

*Implemented: `svi-core/src/state.rs`.*

---

### C2 · Never mistake a market price for a NAV ✅
> **As** a risk lead (P3), **I want** value semantics to be a validated on-chain field **so that** a feed I onboarded as NAV can never silently become a DEX spot price.

**Acceptance criteria**
- ✅ `ValueType` is an on-chain enum: `ProtocolNav`, `Redemption`, `ExchangeRate`, `BackingNav`, `MarketSpot`, `MarketTwap`.
- ✅ `initialize_feed` rejects any discriminant outside 1..=6 (`InvalidValueType`).
- ✅ `MarketSpot` carries an explicit "NEVER use for collateral" annotation in the source of record.
- ⬜ Consumer SDK exposes `require_value_type(...)` as the idiomatic read, so the check is the easy path.

*Implemented: `svi-core/src/state.rs`.*

---

### C3 · Know when the number was true, separately from when it was written ✅
> **As** an on-chain integrator (P4), **I want** `observed_slot` and `published_slot` as distinct fields **so that** I can reason about data age rather than transaction age.

**Acceptance criteria**
- ✅ Both fields stored; `valid_until_slot = observed_slot + max_age_slots` — the window runs from *observation*, not publication.
- ✅ `sequence` is monotonic and enforced on-chain; `sequence == 0` means "never published".
- ✅ `observed_unix_ts` stored for off-chain consumers.

*Implemented: `svi-core/src/lib.rs`, `state.rs`.*

---

### C4 · Detect a methodology change instead of absorbing it silently ⬜
> **As** a risk lead (P3), **I want** the methodology hash on every quote **so that** the definition of a feed I approved cannot change under me.

**Acceptance criteria**
- ✅ `methodology_hash` is stamped on the descriptor and copied onto every quote.
- ⬜ Consumer SDK helper: `require_methodology(expected_hash)`.
- ⬜ Change control documented and enforced: any edit to spec §§2–6 produces a new methodology (`-v2`), a new hash, and **new** quote descriptors — never an in-place redefinition.

*Spec change-control footer.*

---

## Epic D — The verifiable mirror (Pyth's ask, made safe)

### D1 · Give Pyth exactly the API they asked for, doing zero math ⬜
> **As** an oracle integration engineer (P2), **I want** an HTTP endpoint in the shape I requested **so that** integration costs me nothing — and **as** a risk lead I want it to be incapable of inventing a number.

**Acceptance criteria**
- The service performs **no financial arithmetic whatsoever**. Decoding is not arithmetic; deriving, averaging, smoothing or extrapolating is forbidden.
- It reads the quote account via **two independent RPC providers** and requires matching owner, PDA and data hash.
- It verifies descriptor, methodology hash, freshness and status before serving.
- Response includes `observedSlot`, `validUntilSlot`, `sequence`, `quoteAccount`, `adapterProgram`, `methodologyHash`, `status` — so any consumer can re-derive the answer from chain.
- If the two RPCs disagree, or the quote is stale, it serves an explicit error — **never a last-known-good value**.
- A conformance test asserts every served response is byte-reproducible from `getAccountInfo` at `observedSlot`.

*Spec §6; market analysis §1.1. **Blocked on §10 Q3** — Pyth's exact requested feeds, cadence and response schema. Without that answer this is built to a guessed contract, and a formatting mismatch kills the best adoption story we have.*

---

### D2 · Halt the mirror the instant the chain and the observer disagree ⬜
> **As** an auditor (P7), **I want** an independent recomputation that shares no code with the keeper **so that** a bug in the adapter is caught by something that could not share it.

**Acceptance criteria**
- The observer re-executes spec §4 from raw `getMultipleAccounts` at the quote's `observed_slot`, via two independent RPC providers, sharing no code with the keeper beyond `hylo-core` itself.
- Divergence of **even 1 unit** raises an alert and halts the mirror API. Zero tolerance is achievable because §4 is fully deterministic integer math.
- The quote account itself remains readable — the observer gates the *mirror*, never the chain.
- Divergence events are logged with both computations and the full input account set for post-mortem.

*Spec §8.*

---

## Epic E — Operate it

### E1 · Crank permissionlessly, influence nothing 🟡
> **As** a keeper operator (P5), **I want** to refresh any feed without permission, and **as** everyone else I want that keeper to be unable to affect the value.

**Acceptance criteria**
- ✅ Nothing in `publish_quote` authenticates the transaction *payer* — only the adapter PDA.
- ⬜ Reference keeper: refresh every 60s, plus on-deviation trigger at |Δ| > 5 bps vs last written value.
- ⬜ First refresh of a new epoch bundles the `update_lst_prices` CPI.
- ⬜ Refresh transaction measured at **< 200k CU** including the Hylo CPI, on devnet.
- ⬜ Documented runbook so Hylo (or anyone) can co-run a keeper.

*Spec §7. Cadence is `[CONFIRM: Pyth's requested cadence]` — see D1.*

---

### E2 · Retire a feed without breaking consumers ✅
> **As** the SVI operator (P6), **I want** to freeze or deprecate a feed **so that** I can respond to a discovered flaw without an emergency upgrade.

**Acceptance criteria**
- ✅ `set_feed_status` sets `Active | Frozen | Deprecated`, authority-gated via `has_one`.
- ✅ `publish_quote` rejects writes to a non-Active feed (`FeedNotActive`).
- ⬜ Documented deprecation policy: notice period, replacement feed ID, and consumer migration guidance.

*Implemented: `svi-core/src/lib.rs`.*

---

### E3 · Add a protocol without touching shared code ⬜
> **As** the SVI operator (P6), **I want** adding protocol #2 to be "write an adapter and a methodology doc" **so that** the interface is proven to generalise.

**Acceptance criteria**
- A second adapter (LST or Meteora DAMM v2) ships with **zero changes** to `svi-core` and `svi-math`.
- Adapter author guide: required verifications, the CPI-to-accrue pattern, and the methodology-spec template.
- The target protocol is never contacted for a code change.

*This story is the standard's actual proof of existence. Until it passes, SVI is a Hylo wrapper (market analysis §9).*

---

## Epic F — Prove it to a skeptic

### F1 · Reproduce any published number from chain state alone ⬜
> **As** an auditor (P7), **I want** to take a published quote and independently recompute it **so that** trust in SVI is verification, not reputation.

**Acceptance criteria**
- A CLI: given a quote account and a slot, fetch inputs, recompute, and print a byte-for-byte comparison.
- The document defining the computation (`hylo-xsol-nav-v1`) is hashed, frozen, and that hash is on the quote.
- Reproduction requires no SVI-operated infrastructure — public RPC and the open spec suffice.

---

### F2 · Block mainnet on a real test suite ⬜
> **As** the SVI operator (P6), **I want** an explicit pre-mainnet gate **so that** "it worked on devnet" is never the standard.

Pre-mainnet checklist (all must pass before any real collateral depends on a feed):

- [ ] Forged account rejection: wrong owner, wrong PDA, wrong mint, wrong program ID (A1)
- [ ] Unauthorized writer rejection: plain wallet, and a different registered adapter (A5)
- [ ] Monotonicity: replay of an old `observed_slot` is rejected (B1)
- [ ] Freshness: future slot and over-age observations rejected (B1)
- [ ] Bounds: `lower > value` and `value > upper` rejected (A4)
- [ ] Zero-supply path returns exactly 1.000000000 with the flag set (B3)
- [ ] Destabilized path publishes 0 with both halt flags (B3)
- [ ] Epoch-boundary CPI path, on a real devnet epoch rollover (B2)
- [ ] Oracle-degraded path: stale Pyth, wrong feed ID, out-of-tolerance confidence — all abort (B1)
- [ ] Layout-drift path: simulated program upgrade aborts the refresh (B4)
- [ ] Differential test vs. `hylo-core` over ≥1,000 randomized states (A2)
- [ ] Observer parity: zero divergence over ≥72h of continuous devnet operation (D2)
- [ ] Compute budget measured < 200k CU including CPI (E1)
- [ ] `Quote` layout assertion holds; published offsets table matches reality (C1)
- [ ] Third-party audit completed and findings resolved

---

## Non-functional requirements

| ID | Requirement | Rationale |
|---|---|---|
| N1 | No floating point in any valuation path | Determinism; reproducibility by third parties |
| N2 | No panics — every operation total, returning `Option`/`Result` | A panic in a keeper tx is a stale feed; a panic in an atomic-preview consumer tx is a failed liquidation |
| N3 | Refresh tx < 200k CU including CPI | Must fit alongside other instructions |
| N4 | Quote read path cheap enough for a liquidation tx | Drives the zero-copy fixed layout |
| N5 | Core program has no protocol-specific logic | Auditability; eventual freezing |
| N6 | Two independent RPC providers anywhere off-chain data is read | Single-provider trust is one of the five hops SVI exists to remove |
| N7 | Every abort path leaves the quote account byte-identical | "No write" must mean no write |
| N8 | Operating cost < $150/month for the single-protocol pilot | Keeps the project viable without revenue |

---

## Explicitly out of scope for v1

Recording these prevents scope creep and answers "why didn't you…" in review:

| Not building | Why |
|---|---|
| A token, operator network, or slashing | Duplicates the strongest part of Pyth/Switchboard; multiplies complexity and regulatory surface for zero pilot benefit |
| A permissionless publisher registry | Only matters for the "protocols self-register" model. SVI curates the adapter list, exactly like Kamino Scope. A v3 problem, if ever |
| Mint-authority / Token-2022 metadata discovery machinery | Same reason — it exists to solve self-registration, which v1 does not do |
| A generic on-chain formula VM | Enormous attack surface; per-protocol adapters are auditable, a VM is not |
| DEX spot pricing as a collateral feed | Flash-loan manipulable. Where a market value is genuinely needed, read the protocol's own TWAP/observation accounts and tag `MARKET_TWAP` |
| Cross-chain | No demand from the pilot; Solana-only keeps the trust model simple |

# SVI Methodology Specification: `hylo-xsol-nav-v1`

**Status:** Draft v0.9 — pending Hylo review
**Author:** Prince (SVI)
**Date:** 2026-09-01
**Methodology hash:** `sha256(canonical form of this document)` — computed at freeze, embedded in every quote

---

## 1. Purpose

Defines the exact, deterministic procedure by which the SVI Hylo adapter computes and publishes the **net asset value (NAV) of xSOL** from on-chain state, such that any third party can independently reproduce the published value byte-for-byte from the same slot's account data.

Value type: `PROTOCOL_NAV` (redemption-anchored). This is **not** a market price. It is the protocol-defined value of xSOL per Hylo's own redemption math.

## 2. Normative source of math

All arithmetic MUST be performed via the open-source `hylo-core` crate (github.com/hylo-so/sdk), pinned to an exact revision recorded in the adapter's on-chain config:

```
hylo-core rev: [PINNED_GIT_SHA]        # [CONFIRM WITH HYLO: tagged release?]
target program: HYEXCHtHkBagdStcJCp3xbbb9B7sdMdWXFNj6mdsG4hn (Exchange)
```

The adapter does not reimplement Hylo math. It calls:

| Quantity | `hylo-core` function | Rounding (from source) |
|---|---|---|
| xSOL NAV, redeem side | `exchange_math::next_levercoin_redeem_nav` | collateral value: `mul_div_floor` × `price.lower`; NAV: `mul_div_floor` |
| xSOL NAV, mint side | `exchange_math::next_levercoin_mint_nav` | collateral value: `mul_div_ceil` × `price.upper`; NAV: `mul_div_ceil` |
| Collateral ratio | `exchange_math::collateral_ratio` | `mul_div_floor`, `price.lower`; `u64::MAX` if stablecoin supply = 0 |
| hyUSD NAV if depegged | `exchange_math::depeg_stablecoin_nav` | `mul_div_floor` |

Fixed-point domain: `UFix64<N9>` (9 decimals) for NAV and prices, `UFix64<N6>` for token supplies, per `hylo-core` conventions. No floating point anywhere.

## 3. Input accounts

Per `hylo-quotes::protocol_state::ProtocolAccounts`, the SOL-pool computation reads:

| # | Account | Address / derivation | Verified by |
|---|---|---|---|
| 1 | Hylo protocol state | `pda::HYLO` under Exchange program | owner = Exchange, PDA re-derivation |
| 2 | LST headers (per registered LST: jitoSOL, hyloSOL, …) | `pda::lst_header(mint)` | owner + PDA |
| 3 | TotalSolCache (within protocol state) | — | epoch validation, §5.1 |
| 4 | xSOL mint | `4sWNB8zGWHkh6UnmwiEtzNxL4XrN7uK9tosbESbJFfVs` | address allowlist |
| 5 | hyUSD virtual stablecoin state (vUSD_SOL supply) | protocol state | owner + PDA |
| 6 | Pyth SOL/USD `PriceUpdateV2` | per Hylo `OracleConfig` | Pyth program owner + feed ID |
| 7 | Clock sysvar | `SysvarC1ock…` | sysvar address |

The adapter MUST verify owner, PDA derivation, and mint addresses for every account before deserialization. Any mismatch → abort, no write.

`[CONFIRM WITH HYLO]` — canonical enumeration of live LST registry entries and whether the V2 (multi-pool) layout will change any of the above before Q4 2026.

## 4. Computation

Executed atomically within one transaction (all reads same-slot):

```
1. clock            ← Clock sysvar
2. total_sol        ← TotalSolCache.get_validated(clock.epoch)     # §5.1 on failure
3. sol_usd          ← query_pyth_oracle(clock, pyth_feed, hylo_oracle_config)
                       → PriceRange { lower, upper }                # Pyth conf interval,
                                                                    # Hylo's own tolerance config
4. vusd_supply      ← virtual_stablecoin.supply()
5. xsol_supply      ← xsol_mint.supply
6. cr               ← collateral_ratio(total_sol, sol_usd.lower, vusd_supply)
7. zone             ← RebalanceMode::from_cr(cr)                    # §5.2 if Destabilized
8. nav_redeem       ← next_levercoin_redeem_nav(total_sol, sol_usd, vusd_supply, $1, xsol_supply)
9. nav_mint         ← next_levercoin_mint_nav(total_sol, sol_usd, vusd_supply, $1, xsol_supply)
```

Edge cases (all inherited from `hylo-core`, restated here as normative):

- `xsol_supply == 0` → NAV = exactly 1.000000000 (both bounds), flag `ZERO_SUPPLY_DEFAULT`.
- `nav` computes to 0 → `hylo-core` returns `None` → treat as Destabilized path (§5.2).
- `vusd_supply == 0` → CR = ∞ sentinel; NAV computed normally.

## 5. Failure and degraded states

Design rule: **fail stale, never fail wrong.** No code path may publish a value computed from unvalidated inputs.

### 5.1 TotalSolCache outdated

`get_validated` errors with `TotalSolCacheOutdated` when `cache.current_update_epoch != clock.epoch` (i.e., a new epoch began and Hylo's LST-price crank hasn't run). Adapter behavior, in order:

1. Attempt CPI to Exchange `update_lst_prices` (permissionless: payer-only signer) in the same transaction, then re-read and proceed.
2. If the CPI cannot be included (compute budget) or fails: abort without writing. The existing quote ages out via `valid_until_slot` and consumers see it stale.

Note: within an epoch the cache is Hylo's canonical accounting — redemptions settle against it — so quoting it is definitionally correct even though real staking yield accrues continuously. Intra-epoch drift ≤ ~LST APY / epochs-per-year ≈ single basis points.

### 5.2 Destabilized zone (CR < 100%)

Per Hylo semantics: xSOL NAV → 0; mint/redeem halted.

- Publish `quote_amount = lower = upper = 0`.
- Set status flags: `DESTABILIZED | OPERATIONS_HALTED`.
- Additionally refresh the companion `hylo-hyusd-backing-v1` feed using `depeg_stablecoin_nav` so hyUSD consumers see true backing < $1.

A consuming lender MUST treat `DESTABILIZED` as collateral value 0, not as a dip to buy.

### 5.3 Oracle failure

Pyth feed stale, wrong feed ID, or confidence outside Hylo's `OracleConfig` tolerance → abort, no write. (Tolerance is Hylo's on-chain config, read fresh each refresh — SVI imposes no separate threshold, by design: the feed reflects what the protocol itself would accept.)

## 6. Output: quote account mapping

| SVI field | Value |
|---|---|
| `base_mint` | xSOL `4sWN…FfVs` |
| `quote_currency` | USD (SVI currency code 840) |
| `value_type` | `PROTOCOL_NAV` |
| `quote_amount` | `nav_redeem` — the conservative, redemption-anchored value. **This is the number lenders use.** |
| `lower_quote_amount` | `nav_redeem` (floor math, `price.lower`) |
| `upper_quote_amount` | `nav_mint` (ceil math, `price.upper`) |
| `observed_slot` | slot of the refresh transaction (atomic reads ⇒ all inputs this slot) |
| `valid_until_slot` | `observed_slot + validity_window` (default 750 ≈ 5 min) `[CONFIRM WITH HYLO + Pyth cadence]` |
| `sequence` | monotonic, enforced by SVI core |
| `status_flags` | bitfield: `ZERO_SUPPLY_DEFAULT`, `DESTABILIZED`, `OPERATIONS_HALTED`, `SELL_ZONE`, `BUY_ZONE`, `EPOCH_BOUNDARY_CPI` (set when §5.1 step 1 ran) |
| `methodology_hash` | hash of this document, frozen |

Rationale for `quote_amount = nav_redeem`: Hylo itself prices NAV as a range and always takes the side favorable to the protocol (mint: ceil/upper; redeem: floor/lower). A holder liquidating xSOL realizes the redeem side; therefore redeem NAV is the defensible collateral value. Publishing the range preserves full information for consumers with other needs.

## 7. Update policy

- **Cadence:** refresh every 60 s, plus on-deviation trigger at |Δ| > 5 bps vs last written value. `[CONFIRM: Pyth's requested cadence]`
- **Epoch boundary:** first refresh of a new epoch bundles the `update_lst_prices` CPI (§5.1).
- **Cranking:** permissionless. Reference keeper operated by SVI during pilot; Hylo expected to co-run post-integration.
- **Compute budget:** refresh tx target < 200k CU including Hylo CPI. `[MEASURE on devnet]`

## 8. Independent verification (observer)

An off-chain observer, sharing no code with the keeper beyond `hylo-core` itself, re-executes §4 from raw `getMultipleAccounts` at the quote's `observed_slot` via two independent RPC providers and compares to the published quote. Divergence of even 1 unit → alert + mirror API halt. Zero tolerance is achievable because §4 is fully deterministic integer math.

## 9. Companion feeds (same adapter, same account set)

| Feed | Formula source | Notes |
|---|---|---|
| `hylo-hyusd-backing-v1` | master equation Σ(vUSDᵢ) vs supply; `depeg_stablecoin_nav` when CR < 100% | backing ratio + NAV |
| `hylo-ehyusd-rate-v1` | `earn_pool_math` (hyUSD in pool / eHYUSD supply, zero → $1) | pool token ATA + sHYUSD mint |
| `hylo-xsol-leverage-v1` | TVL / (NAV × supply) | informational; not a price |

## 10. Open questions for Hylo (all remaining)

1. **V2 timing:** equations doc says V2; addresses page lists V1 programs; SDK contains exo/cbBTC/HYPE machinery. Which layout is live on mainnet today, and is any account structure above changing before Nov 2026?
2. **`update_lst_prices` in CPI:** any constraint that prevents invocation via CPI from a third-party program (e.g., LUT requirements, remaining-accounts size)?
3. **Pyth's exact request:** which feeds, cadence, and response schema did Pyth ask for? (Defines the mirror API contract and §7 cadence.)
4. **Pinning:** will Hylo tag SDK releases aligned to program deployments so the adapter can pin `hylo-core` to the deployed program version?
5. **Blessing:** may SVI list Hylo as design partner for the Colosseum submission, and will Hylo co-announce the feed?

---

*Change control: any edit to §§2–6 is a new methodology (`hylo-xsol-nav-v2`), a new hash, and new quote descriptors. §7 parameters may be tuned via adapter config without version bump; §10 answers get folded in before freeze.*

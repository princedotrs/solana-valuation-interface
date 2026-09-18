# SVI — Product Architecture

**Solana Valuation Interface** · v1 architecture · September 2026

> **One sentence:** the value of a protocol asset is computed by a program
> *on-chain*, from accounts anyone can verify, and written to a public account
> anyone can read — so no off-chain service ever has to be trusted with the
> arithmetic.

---

## 1. System context

Gray = already exists, SVI touches nothing. Purple = SVI builds and deploys.
Amber = the product. Blue = consumers.

```mermaid
graph TB
    subgraph SRC["Existing on-chain state — unmodified, no cooperation required"]
        H["Hylo Exchange program<br/>protocol state · LST headers<br/>TotalSolCache · xSOL mint"]
        S["Sanctum / SPL stake pools<br/>LST redemption rates"]
        P["Pyth SOL/USD<br/>PriceUpdateV2"]
        C["Clock sysvar"]
    end

    subgraph SVI["SVI — deployed programs"]
        A["<b>svi-hylo-adapter</b><br/>the calculator<br/>verifies accounts → calls hylo-core<br/>→ CPI publish"]
        M["<b>svi-math</b><br/>exact integer math<br/>explicit rounding, no floats"]
        CORE["<b>svi-core</b><br/>the notice board<br/>dumb, auditable, freezable"]
    end

    Q["<b>Quote account</b> — 320 bytes, fixed layout<br/>value + lower/upper bounds · observed_slot<br/>valid_until_slot · sequence · status_flags<br/>methodology_hash · value_type"]

    subgraph OPS["Operations"]
        K["Keeper<br/>permissionless crank<br/>~60s + 5bps deviation"]
        O["Independent observer<br/>recomputes from raw accounts<br/>via 2 RPCs · halts mirror on divergence"]
    end

    subgraph CONS["Consumers"]
        MIR["Verifiable mirror API<br/><b>zero arithmetic</b><br/>reads account via 2 RPCs, requires<br/>byte-for-byte match, decodes, serves"]
        PY["Pyth publisher"]
        LEND["Lending markets<br/>Kamino · marginfi · Loopscale"]
        SCOPE["Kamino Scope"]
        UI["Wallets · dashboards"]
    end

    H -->|"accounts passed<br/>into the tx"| A
    S --> A
    P --> A
    C --> A
    M -.->|"linked crate"| A
    A -->|"CPI, signed by<br/>adapter PDA"| CORE
    CORE -->|"the only writer"| Q
    K -->|"pays gas, cannot<br/>influence the value"| A
    Q --> O
    H -.->|"independent<br/>re-read"| O
    Q --> MIR
    Q -->|"direct account read<br/>~few hundred CU"| LEND
    Q --> SCOPE
    Q --> UI
    MIR --> PY
    O -.->|"halt on divergence"| MIR

    classDef ext fill:#e8e8ec,stroke:#8b8b96,color:#1a1a1f
    classDef svi fill:#e9defa,stroke:#7c3aed,color:#1a1a1f
    classDef prod fill:#fde9c8,stroke:#d97706,color:#1a1a1f
    classDef cons fill:#d7e9fb,stroke:#2563eb,color:#1a1a1f
    classDef ops fill:#e2f5e9,stroke:#16a34a,color:#1a1a1f
    class H,S,P,C ext
    class A,M,CORE svi
    class Q prod
    class MIR,PY,LEND,SCOPE,UI cons
    class K,O ops
```

### The five pieces, in plain language

| Piece | Analogy | Job |
|---|---|---|
| **Adapter** (`svi-hylo-adapter`) | the robot accountant | One per protocol. Knows *this* protocol's formula and account layout. Verifies every input account (owner, PDA derivation, mint, program ID), computes the value, refuses to work with unverified receipts. |
| **Core** (`svi-core`) | the rule that only the accountant may post to the wall | Tiny, protocol-agnostic, deliberately dumb. Knows nothing about Hylo. Enforces: registered adapter PDA signed it, sequence increases, observation is fresh and newer than what's stored, bounds are coherent. Dumb is auditable — and one day freezable. |
| **Quote account** | the audited statement posted on the wall | **This is the product.** 320 fixed bytes: the value, its bounds, when it was observed, when it expires, and a hash of exactly which formula produced it. |
| **Keeper** | the person who rings the bell | Sends the refresh transaction. Permissionless — anyone can crank. Pays gas; **cannot influence the number**. |
| **Observer** | the independent second accountant | Re-derives the value from raw accounts through separate RPCs and compares. Any divergence → alert + mirror halt. |

**The principle the whole design reduces to:** there is no point in the
pipeline where a human or a hackable server *chooses* the number. The number is
a pure function of on-chain state, computed by public code; everything
downstream only transports it.

---

## 2. Trust boundaries — before and after

```mermaid
graph LR
    subgraph BEFORE["Today: what Pyth asked Hylo to build"]
        direction TB
        b1["Hylo accounts"] --> b2["one RPC"] --> b3["private parser"]
        b3 --> b4["private math"] --> b5["Hylo API"] --> b6["publisher key"] --> b7["price on-chain"]
    end
    subgraph AFTER["With SVI"]
        direction TB
        a1["Hylo accounts"] --> a2["adapter program<br/>public code, on-chain math"]
        a2 --> a3["quote account<br/>canonical, public"]
        a3 --> a4["mirror API<br/>zero math, byte-verifiable"]
        a4 --> a5["publisher key"] --> a6["price on-chain"]
    end

    classDef bad fill:#fde2e2,stroke:#dc2626,color:#1a1a1f
    classDef good fill:#e2f5e9,stroke:#16a34a,color:#1a1a1f
    class b2,b3,b4,b5,b6 bad
    class a2,a3,a4 good
```

Six trusted hops become one — and that one (the adapter) is public code whose
output any third party can reproduce byte-for-byte from the same slot's
account data.

**What SVI does not fix:** Pyth's publisher key management (hop `a5`) is
still Pyth's. SVI removes *hidden pricing logic*, not *all* infrastructure
risk. Claiming otherwise would be dishonest and Plish would catch it.

---

## 3. How a value gets refreshed

One transaction, all reads same-slot-consistent by construction.

```mermaid
sequenceDiagram
    autonumber
    participant K as Keeper (anyone)
    participant AD as svi-hylo-adapter
    participant HY as Hylo Exchange
    participant PY as Pyth PriceUpdateV2
    participant CO as svi-core
    participant Q as Quote account

    K->>AD: refresh_quote(accounts…)
    Note over AD: Verify EVERY account:<br/>owner == Exchange program,<br/>PDA re-derivation, mint allowlist,<br/>Pyth program owner + feed ID.<br/>Any mismatch → abort, no write.
    AD->>HY: deserialize protocol state, LST headers,<br/>TotalSolCache, xSOL mint supply
    alt TotalSolCache epoch != clock.epoch
        AD->>HY: CPI update_lst_prices (permissionless, payer-only signer)
        Note over AD: set EPOCH_BOUNDARY_CPI flag,<br/>re-read cache
    end
    AD->>PY: read price + confidence → PriceRange{lower, upper}
    Note over AD: hylo-core (pinned SHA), integer only:<br/>cr = collateral_ratio(…)<br/>nav_redeem = next_levercoin_redeem_nav(…)  [floor, price.lower]<br/>nav_mint   = next_levercoin_mint_nav(…)    [ceil,  price.upper]
    alt CR < 100% (Destabilized)
        AD->>CO: publish 0 with DESTABILIZED | OPERATIONS_HALTED
    else Oracle stale / out of Hylo's tolerance
        AD--xK: abort — no write. Quote ages out via valid_until_slot.
    else Healthy
        AD->>CO: CPI publish_quote(update), signed by adapter PDA
    end
    CO->>CO: feed Active? · signer == registered adapter_authority?<br/>lower ≤ value ≤ upper? · observed_slot ≤ clock.slot?<br/>age ≤ max_age_slots? · observed_slot > stored?
    CO->>Q: write value, bounds, slots, sequence+1, flags
    CO-->>K: emit QuotePublished
```

**The security property in one line:** *anyone* can send this transaction, but
*nobody* can influence what value it writes. The keeper pays gas. The value is
computed by adapter code from verified accounts, and only the adapter's PDA can
authorize the write to core. If the keeper dies, anyone else's bot cranks it.
If nobody cranks it, the quote goes stale and `valid_until_slot` makes that
visible — it fails loud, it never lies.

---

## 4. How a value gets consumed

```mermaid
graph TB
    Q["Quote account (320 bytes)"]

    subgraph A["Mode A — read the note · 99% of usage"]
        A1["Lender's program passes the quote<br/>account into its own instruction"]
        A2["Check valid_until_slot > clock.slot<br/>Check status_flags healthy<br/>Check value_type == PROTOCOL_NAV<br/>Check methodology_hash == expected"]
        A3["Use lower_quote_amount<br/>for collateral"]
        A1 --> A2 --> A3
    end

    subgraph B["Mode B — call the calculator live · high-stakes only"]
        B1["Liquidator includes the adapter's<br/>compute in their own transaction"]
        B2["Value computed from THIS slot's state,<br/>atomically, alongside the liquidation"]
        B3["More accounts, more CU,<br/>zero staleness"]
        B1 --> B2 --> B3
    end

    subgraph C["Mode C — off-chain mirror"]
        C1["Fetch account via 2 independent RPCs"]
        C2["Require byte-for-byte agreement"]
        C3["Decode, serve as JSON with provenance"]
        C1 --> C2 --> C3
    end

    Q --> A1
    Q -.-> B1
    Q --> C1

    classDef m fill:#d7e9fb,stroke:#2563eb,color:#1a1a1f
    classDef prod fill:#fde9c8,stroke:#d97706,color:#1a1a1f
    class Q prod
    class A1,A2,A3,B1,B2,B3,C1,C2,C3 m
```

The mirror API is a **photocopier**. It cannot lie about the price, because
anyone can compare its output against the account it copied. That is what turns
Pyth's "give us an API" request into something safe to say yes to — Pyth gets
exactly the API it asked for, and the API does no math.

**Compute budget note:** an xSOL NAV computation touching multiple LST headers
plus a Pyth read costs tens of thousands of CU. Fine in a dedicated keeper
transaction; often *not* fine inside a consumer's liquidation transaction.
Design for Mode A as the default read path — that is why the stored quote
account, not the atomic preview, is the product.

---

## 5. Data model

```mermaid
classDiagram
    class Descriptor {
        +feed_id: [u8;32]  «sha256 of feed name»
        +authority: Pubkey  «may freeze/retire»
        +adapter_program: Pubkey
        +adapter_authority: Pubkey  «THE security boundary»
        +adapter_config: Pubkey  «pinned SDK rev, program hash»
        +base_mint / quote_mint: Pubkey
        +methodology_hash: [u8;32]
        +max_age_slots: u64
        +quote_currency_code: u16  «840 = USD»
        +value_type: u8
        +base_decimals / quote_decimals: u8
        +status: u8  «Active|Frozen|Deprecated»
    }
    class Quote {
        «320 bytes, layout-asserted at compile time»
        +descriptor / feed_id / base_mint / quote_mint
        +adapter_program: Pubkey
        +methodology_hash: [u8;32]
        +source_accounts_hash: [u8;32]
        +base_amount: u64
        +quote_amount: u64  «the number lenders use»
        +lower_quote_amount: u64  «redeem NAV, floor»
        +upper_quote_amount: u64  «mint NAV, ceil»
        +observed_slot: u64
        +observed_unix_ts: i64
        +published_slot: u64
        +valid_until_slot: u64
        +sequence: u64  «monotonic»
        +status_flags: u64  «bitfield»
    }
    class ValueType {
        <<enumeration>>
        ProtocolNav = 1
        Redemption = 2
        ExchangeRate = 3
        BackingNav = 4
        MarketSpot = 5  «NEVER for collateral»
        MarketTwap = 6
    }
    Descriptor "1" --> "1" Quote : governs
    Descriptor --> ValueType : declares
```

Two decisions worth defending:

**Why `base_amount → quote_amount` instead of a decimal price?** A price like
"28.43125" forces every consumer to agree on a decimal convention and invites
float parsing. `1_000_000_000 base units = 28_431_250 quote units` is exact,
integer, and self-describing alongside `base_decimals` / `quote_decimals`.

**Why is `value_type` an on-chain enum rather than documentation?** So a
consumer that asked for NAV can never silently receive a market price. It is
validated in `initialize_feed` and it is the difference between the Moonwell
incident happening to an SVI consumer and not.

---

## 6. Failure states — *fail stale, never fail wrong*

The design rule: **no code path may publish a value computed from unvalidated
inputs.** Degraded is always preferable to plausible-but-wrong.

```mermaid
stateDiagram-v2
    [*] --> Healthy
    Healthy --> Healthy : refresh OK<br/>sequence++, flags clear
    Healthy --> EpochBoundary : TotalSolCache epoch != clock.epoch
    EpochBoundary --> Healthy : CPI update_lst_prices succeeds<br/>flag EPOCH_BOUNDARY_CPI
    EpochBoundary --> Stale : CPI can't be included (CU) or fails<br/>ABORT, no write
    Healthy --> Destabilized : CR < 100%
    Destabilized --> Destabilized : publish 0<br/>DESTABILIZED | OPERATIONS_HALTED
    Destabilized --> Healthy : CR recovers
    Healthy --> Stale : Pyth stale / wrong feed / confidence<br/>outside Hylo's own tolerance<br/>ABORT, no write
    Healthy --> Stale : source program upgraded<br/>(programdata hash changed)
    Stale --> Healthy : inputs valid again
    Stale --> [*] : valid_until_slot passes<br/>consumers see expiry, use nothing
    Healthy --> Frozen : admin set_feed_status
    Frozen --> [*] : no writes accepted
```

| Condition | Behaviour | Consumer sees |
|---|---|---|
| Everything healthy | publish, `sequence++` | fresh quote, flags clear |
| New epoch, LST crank not run | bundle Hylo's permissionless `update_lst_prices` CPI, then publish | `EPOCH_BOUNDARY_CPI` set |
| …and the CPI won't fit / fails | **abort, no write** | quote ages past `valid_until_slot` |
| CR < 100% (Destabilized) | publish **0**, halt flags, refresh hyUSD backing feed | `DESTABILIZED \| OPERATIONS_HALTED` — collateral value **zero**, not a dip to buy |
| Pyth stale / wrong feed / confidence outside Hylo's `OracleConfig` | **abort, no write** | stale quote, expiry visible |
| xSOL supply == 0 | NAV = exactly 1.000000000 both bounds | `ZERO_SUPPLY_DEFAULT` |
| Target program upgraded (layout drift) | **abort, no write** until reviewed | stale quote, expiry visible |
| Observer disagrees with chain | alert + **mirror API halts** | mirror stops serving; account still readable |

SVI imposes **no separate oracle tolerance** — it reads Hylo's own on-chain
`OracleConfig` fresh on every refresh. By design: the feed reflects what the
protocol itself would accept for a real redemption. SVI is not entitled to a
second opinion about Hylo's risk parameters.

---

## 7. Adding a protocol — why this generalises

```mermaid
graph LR
    subgraph SHARED["Built once, shared by every protocol"]
        CORE["svi-core"]
        MATH["svi-math"]
        MIRROR["mirror API"]
        OBS["observer"]
        SDK["consumer SDKs"]
    end
    subgraph PER["Per protocol — the only new work"]
        AD1["svi-hylo-adapter<br/>+ methodology spec"]
        AD2["svi-lst-adapter<br/>Sanctum / SPL stake pools"]
        AD3["svi-meteora-adapter<br/>DAMM v2 / DLMM"]
        AD4["svi-kamino-adapter<br/>kTokens"]
    end
    AD1 --> CORE
    AD2 --> CORE
    AD3 --> CORE
    AD4 --> CORE
    CORE --> MIRROR
    CORE --> OBS
    CORE --> SDK

    classDef s fill:#e2f5e9,stroke:#16a34a,color:#1a1a1f
    classDef p fill:#e9defa,stroke:#7c3aed,color:#1a1a1f
    class CORE,MATH,MIRROR,OBS,SDK s
    class AD1,AD2,AD3,AD4 p
```

Supporting a new protocol means: write one adapter that knows that protocol's
account layout, register its PDA as the authority for those feeds, point a
keeper at it, publish a methodology spec. **The target protocol never hears
from us.** Core, quote format, mirror, observer and SDKs are all shared.

---

## 8. Feed set — Hylo

Never one ambiguous feed called `xSOL/USD`. Value semantics are explicit,
per-feed, on-chain.

| Feed ID | Value type | Formula source | Status |
|---|---|---|---|
| `hylo-xsol-nav-v1` | `PROTOCOL_NAV` | `next_levercoin_redeem_nav` / `_mint_nav` | **spec drafted v0.9** |
| `hylo-hyusd-backing-v1` | `BACKING_NAV` | master equation Σ(vUSDᵢ); `depeg_stablecoin_nav` when CR < 100% | companion |
| `hylo-ehyusd-rate-v1` | `EXCHANGE_RATE` | `earn_pool_math` (hyUSD in pool / sHYUSD supply; zero → $1) | companion |
| `hylo-xsol-leverage-v1` | informational | TVL / (NAV × supply) | not a price |
| `hylo-xsol-redemption-v1` | `REDEMPTION` | redeem NAV **after protocol fees** | v1.1 |
| `hylo-hyusd-usdc-redemption-v1` | `REDEMPTION` | including applicable fee | v1.1 |

**V2 scaling:** the xAsset Engine turns this from a fixed list into
`4 feeds × N xAssets`. Each new xAsset is a methodology document and a config
entry, not a new API.

---

## 9. Build status

| Component | State | Notes |
|---|---|---|
| `crates/svi-math` | **Done** | Exact `u128`-intermediate integer math, mandatory explicit rounding, `no_std`, `#![deny(clippy::arithmetic_side_effects)]`, property tests |
| `programs/svi-core` | **Done** | Descriptor + 320-byte zero-copy Quote with compile-time layout assertion, `publish_quote` with the 4 rules, events, admin freeze |
| `docs/hylo-xsol-nav-v1-spec.md` | **Draft v0.9** | Pending Hylo answers to §10; 5 open questions |
| `programs/svi-hylo-adapter` | **Scaffolded** | Anchor skeleton in place; real instruction blocked on the §10 answers |
| `programs/svi-mock-adapter` | **Done** | Exercises the core's CPI + authority path end-to-end |
| Keeper | Not started | Phase 2 |
| Mirror API + observer | Not started | Phase 2 |

The critical-path dependency is not engineering capacity. It is **five
answers from Hylo** (spec §10) — which is the entire ask.

---

## 10. Design decisions and their rationale

| Decision | Why |
|---|---|
| Adapter reads accounts directly; no CPI to read | Solana has no view functions, and none are needed — account bytes are public and same-slot consistent within a transaction. Removes all publisher-side cooperation. |
| CPI only to *accrue* (`update_lst_prices`) | Some state is stale until cranked. Hylo's crank is permissionless (payer-only signer), so this needs no permission either. |
| Depend on `hylo-core`, reimplement nothing | Eliminates the single largest correctness risk — divergence between SVI's math and the protocol's own. Pin to an exact git SHA recorded on-chain in `adapter_config`. |
| `quote_amount = nav_redeem` (the conservative side) | Hylo prices NAV as a range and always takes the side favourable to the protocol. A holder liquidating xSOL realises the *redeem* side; therefore redeem NAV is the defensible collateral value. Publishing the full range preserves information for other consumers. |
| Rounding direction is a mandatory parameter, never a default | Round *against* whoever benefits: collateral down, debt up. A classic exploit family lives in the other choice. |
| Integer only, `u128` intermediates, `Option` returns | No floats anywhere. Every operation is total — no panics in the valuation path. |
| Core is deliberately dumb | Auditable, and eventually freezable. All protocol knowledge lives in swappable adapters. |
| `observed_slot` separate from `published_slot` | A consumer must be able to distinguish "when was this true" from "when was this written". |
| `methodology_hash` on every quote | Changing §§2–6 of a methodology produces a new hash and new feed — a consumer's expectations cannot silently change under it. |
| Store target program's deployed hash in `adapter_config` | Layout drift becomes a loud abort instead of a silent misprice. |
| No token, no operator network, no slashing | Duplicates the strongest part of Pyth/Switchboard while multiplying complexity and regulatory surface. |

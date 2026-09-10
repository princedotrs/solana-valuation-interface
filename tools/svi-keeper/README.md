# svi-keeper

The permissionless keeper for the `hylo-xsol-nav-v1` feed, and a watcher for
what it publishes. One binary, two subcommands.

```bash
cargo build --manifest-path tools/svi-keeper/Cargo.toml
K=tools/svi-keeper/target/debug/svi-keeper

$K crank [--interval 5] [--surfnet]     # keep the quote fresh
$K watch                                # our quote, redrawn every second
$K watch --pyth                         # the input it is derived from
```

Flags override `SVI_RPC`, `SVI_KEYPAIR`, `SVI_MAINNET_RPC` (surfnet mode only).
RPC URLs are never printed; they may carry API keys.

## Why anyone can run it

The keeper holds no privileged key. `svi-core` accepts the value because the
adapter's own PDA signed the CPI, not because of who paid for the transaction.
Run it from any wallet; run three of them; the quote is the same. A refusal
(stale oracle, cache epoch, band too wide) is logged with its code and the
loop continues — the previous quote simply ages past `valid_until_slot`.
*Fail stale, never fail wrong.*

## Surfnet mode

A Surfnet clones the Pyth account once at fork and never updates it, while
its clock keeps running; Hylo's oracle window is 10 seconds. `--surfnet` makes
each tick re-clone Hylo's three accounts from mainnet and pin the Surfnet clock
to that snapshot's own slot and time for the one instruction that reads it —
what `programs/svi-hylo-adapter/tests/surfnet.rs` does, in a loop. No Hylo or
Pyth byte is altered. On mainnet none of this applies.

## The demo layout

```
┌──────────────────────────────┬──────────────────────────────┐
│ $K watch                     │ $K watch --pyth              │
│ SVI hylo-xsol-nav-v1         │ PYTH SOL/USD (the input)     │
│ NAV $0.060285498 per xSOL    │ price $ 212.4310 ± 0.09      │
│ bounds .. (4.9 bps)          │ publish_time … (3s ago)      │
│ age 7 slots  VALID           │                              │
├──────────────────────────────┴──────────────────────────────┤
│ $K crank --interval 5                                       │
│ 14:02:11Z published $0.060285498 [..] seq 41 slot … BUY_ZONE│
└─────────────────────────────────────────────────────────────┘
```

Frame it correctly. Pyth publishes SOL/USD, the *input*; nobody publishes xSOL
NAV, the *output*, and our quote can never be fresher than the input it is
derived from (hylo-core refuses anything older than 10 s). The claim the
screen supports is: the protocol's own valuation, on-chain, in a public
account, moving ~2.5× SOL, with bounds and a staleness horizon, written only
by code. A third pane showing the xSOL *market* price (Jupiter) would add the
"why a lender must not use the DEX price" argument; it is not built yet.

## Mainnet

The keeper works against any cluster where both programs are deployed. Before
a real deployment, set `methodology_hash` to the frozen spec's hash (it is
zero) and confirm `max_age_slots` against Pyth's actual cadence.

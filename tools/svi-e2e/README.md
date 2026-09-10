# svi-e2e

Publish a real quote and read it back. This is the step that turns "the program
compiles" into "the program works".

## Run it

Three shells, in order.

```bash
# 1. a validator carrying Hylo's REAL mainnet accounts
./scripts/localnet.sh 'https://your-mainnet-rpc'

# 2. deploy both programs to it
./scripts/deploy-local.sh

# 3. initialize the feed, refresh it, read the quote back
cargo run --manifest-path tools/svi-e2e/Cargo.toml
```

## What it exercises

The whole chain, for real: the adapter reads Hylo's accounts, verifies owner
and PDA derivation, computes NAV with `hylo-core`, CPIs into `svi-core` signed
by its authority PDA, and `svi-core` writes the 320-byte quote account. Then
this tool reads that account back **by byte offset** — the same way a consuming
lender would — and cross-checks the published value against an independent
off-chain computation over the same state.

Nothing is mocked. `scripts/localnet.sh` clones the accounts from mainnet, so
the program runs on the same bytes mainnet has.

## The epoch subtlety, which is worth seeing

A fresh validator starts at slot 0, epoch 0. Hylo's `TotalSolCache` is stamped
with the epoch it was computed in, and `hylo-core` refuses to compute when
those disagree. So by default the script warps to the current mainnet slot,
which reproduces the mainnet epoch (432,000 slots per epoch).

Run it once without warping:

```bash
WARP=0 ./scripts/localnet.sh 'https://your-mainnet-rpc'
```

The refresh should **fail** with `ContextUnavailable`. That is the design
working: an epoch boundary where Hylo's LST crank has not run is exactly the
case where a naive adapter would publish a stale NAV, and this one refuses.
*Fail stale, never fail wrong* — observed rather than asserted.

## Reading the output

`sequence` starts at 1 on the first publish and increments on every refresh;
`sequence == 0` would mean nothing was ever written. `status_flags` is a
bitfield — `0` is healthy, bit 1 is `DESTABILIZED`, bit 3/4 are the sell/buy
zones. `valid_until_slot` is when consumers must stop trusting the value.

The `MATCH` line at the end compares the on-chain published number against one
computed off-chain from the same accounts. They can differ legitimately if
state moved between the two reads; a persistent difference is a bug worth
chasing.

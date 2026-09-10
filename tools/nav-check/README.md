# nav-check

Reproduce Hylo's xSOL NAV from mainnet state, independently, in one command.

```bash
cargo run                          # uses public mainnet RPC
cargo run -- https://your-rpc-url  # or pass your own (recommended)
RPC_URL=https://your-rpc-url cargo run
```

Compare the printed **redeem NAV** against the xSOL price on
[hylo.so](https://hylo.so). A match is the whole technical thesis of SVI,
demonstrated rather than argued.

## What this proves

1. **The value is derivable from public state.** Four accounts — Hylo's
   protocol state, the xSOL mint, the Pyth SOL/USD feed, and the Clock sysvar —
   read in one call, at one slot. No API, no private arithmetic, no permission
   from anyone.
2. **SVI reimplements none of Hylo's math.** Every number comes out of
   `hylo-core`, pinned to an exact git revision in `Cargo.toml`. SVI's published
   value and Hylo's redemption math cannot disagree, because they are the same
   code.
3. **The failure modes are the right ones.** At an epoch boundary, before Hylo's
   LST crank has run, `fetch_lst_context` returns `TotalSolCacheOutdated` and
   this tool exits with that error rather than printing a plausible wrong
   number. That is *fail stale, never fail wrong* (methodology spec §5.1),
   inherited from Hylo's own library rather than invented here.

## What it does not prove

It does not prove anything about the on-chain program: this runs off-chain and
reads through one RPC. It is the arithmetic thesis, not the trust thesis. The
trust properties — same-slot atomic reads, adapter-PDA-only writes, the
canonical quote account — come from `programs/svi-core` and the adapter, and are
what the independent observer (spec §8) is later built to check against.

## Mapping to the methodology spec

| This tool prints | Spec §6 field | Source |
|---|---|---|
| redeem NAV — floor math, `price.lower` | `quote_amount` and `lower_quote_amount` | `exchange_math::next_levercoin_redeem_nav` |
| mint NAV — ceil math, `price.upper` | `upper_quote_amount` | `exchange_math::next_levercoin_mint_nav` |
| hyUSD NAV | `hylo-hyusd-backing-v1` feed | `ExchangeContext::stablecoin_nav` |
| collateral ratio | `status_flags` zone bits | `ExchangeContext::collateral_ratio` |

## Note on the lockfile

`Cargo.lock` is taken from the Hylo SDK at the pinned revision. The Pyth
receiver SDK and Anchor are version-sensitive here — resolving them freshly
pulls a mismatched `anchor-lang` and fails to build. Keep the lockfile.

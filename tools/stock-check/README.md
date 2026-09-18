# stock-check

The second opinion on a published stock quote.

```bash
cargo run --manifest-path tools/stock-check/Cargo.toml -- AAPL
cargo run --manifest-path tools/stock-check/Cargo.toml -- --self-test
```

It reads the two published quote accounts straight from the chain, asks Pyth's
Hermes independently what the underlying feeds say, recomputes what the quotes
should contain, and prints both.

## Why it is a second opinion and not an echo

It depends on **none** of SVI's crates. Not `svi-math`, not the adapter, not
the keeper. The scaling arithmetic is written out again, by hand, from Pyth's
documented exponent convention.

Its self-test then pins sixteen vectors against `svi-math` — the code the
on-chain adapter actually runs. Two implementations agreeing is evidence; one
implementation agreeing with itself is not. If the two ever diverge, one of
them has a bug and that test is where it surfaces.

## What it will complain about

- a quote whose value is outside its own published bounds
- a `base_amount` of zero, which makes the quote meaningless
- the fair quote not typed `ReferenceFairValue` (7), or the market quote not
  `MarketSpot` (2)
- the pair not written at the same slot, which would mean they were not
  written by the same instruction
- a payload shorter than the full 320 bytes

It also prints the chain's current slot next to the quote's `valid_until_slot`,
so an answer from a lagging RPC looks as old as it is.

## What it will not do

Assert that the published number equals what Hermes says right now. It cannot:
Hermes is answering for this instant and the quote was published at the slot it
names. The dollar figures are printed side by side for you to judge; a large
gap on a fresh quote is the thing to look at.

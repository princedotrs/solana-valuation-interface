# SVI for tokenized stocks

**The problem, the fix, and what actually exists** · September 2026

*If you read one document, read this one.*

---

## AAPLx trades all night. Apple doesn't.

A tokenized stock like **AAPLx** trades on Solana twenty-four hours a day,
seven days a week. The share it represents trades six and a half hours a day,
five days a week, and is shut for every US public holiday.

So for roughly **two thirds of every week**, the token has a price and the
company does not. Thin overnight books, a rumour, one large seller — the token
moves, and there is no market on earth willing to arbitrage it back, because
the thing it tracks is closed.

Nothing on-chain says any of this. A lending market that reads the token's
midnight price reads a number, with no indication that the number is a guess
made in an empty room. It liquidates against it anyway.

## What SVI publishes

For each stock, **one transaction** reads two Pyth feeds and writes two
accounts:

| account | what it holds | where it comes from |
|---|---|---|
| `stock-aapl-fair-value-v1` | what the **real share** is worth | Pyth `Equity.US.AAPL/USD` |
| `stock-aapl-market-v1` | what the **token** trades at on Solana | Pyth `Crypto.AAPLX/USD` |

Both are written in the same instruction, so the pair can never be
half-updated: you cannot read a fresh market price against a stale fair value,
because there is no moment at which one exists without the other.

Each account carries the value, Pyth's confidence interval as explicit
`lower`/`upper` bounds, the slot it was observed at, the slot it expires at,
and a set of flags.

### The flags are the product

The value is the easy part. What makes this worth deploying is that the
account *tells you what kind of moment it was taken in*:

| flag | raised when | what a consumer should do |
|---|---|---|
| `MARKET_CLOSED` | the equity price is over 15 minutes old — impossible during a session, so it means the exchange is shut | widen your margins; the fair value is the last real print, not a live one |
| `REFERENCE_STALE` | over 4 days old, which no ordinary weekend or holiday explains | stop trusting the fair value |
| `TOKEN_FEED_STALE` | the token feed has gone quiet, which it never should | something is wrong with the oracle, not the market |
| `DEVIATION_HIGH` | the token is more than 2% away from the real share | the token is dislocated; do not treat its price as the asset's value |

A flag is a thing worth reporting. A **refusal** is different. Past eight days
with no equity price, or with a confidence interval wider than 5%, the adapter
publishes nothing at all — and the previous quote simply expires.

That rule has a name: **fail stale, never fail wrong.** A consumer that sees an
expired quote knows to stop. A consumer that sees a plausible wrong one does
not.

### Why two windows per feed, and not one

This is the design decision the whole product turns on.

"Too old to use" and "old enough to mention" are different questions, and
collapsing them makes the product impossible. An equity feed is *supposed* to
be hours old overnight. If the adapter refused to publish whenever the
reference was stale, the feed would go dark at exactly the moment a consumer
most needs to be told that the market is shut.

So: 15 minutes raises `MARKET_CLOSED`, four days adds `REFERENCE_STALE`, eight
days refuses. Three thresholds, all written in the symbol's on-chain config,
where a third party can read the rules that produced a quote rather than
trusting a description of them.

## Why you can believe it

**There is no API.** The math runs in a Solana program. The inputs are Pyth
accounts. The output is an account you read directly. Nothing in the path is a
server anybody operates.

**The keeper cannot lie.** Anyone can crank the feed — it is permissionless and
holds no privileged key. `svi-core` accepts the write only because the
*adapter program's own PDA* signed the CPI, and the adapter computes the value
from accounts it verified itself. A keeper chooses which accounts go into a
transaction and nothing else.

**Substituting a feed fails loudly.** The 32-byte Pyth feed id inside each
price account is compared, on-chain, on every refresh, against the one in the
symbol's config. Without that check, anyone could pass a real, fully verified
Pyth price for a *different, cheaper* asset and have it published as Apple's.

**You can re-run it.** `tools/stock-check` reads the published accounts, asks
Pyth independently what the feeds say, and disagrees out loud if they do not
match. It depends on none of SVI's crates — its arithmetic is written out
again from Pyth's documented convention, and its tests pin sixteen vectors
against the code the adapter actually runs. Two implementations agreeing is
evidence.

## Who needs this

| | what they read | what it stops |
|---|---|---|
| **Lending markets** | `lower` for collateral, `MARKET_CLOSED` to widen margins | liquidating someone against a 3am price no exchange agrees with |
| **Perp and options venues** | both feeds, and `DEVIATION_HIGH` | marking a book to a dislocated token |
| **Structured products** | the fair value, when the token is the wrong reference | settling on a price that reflects thin overnight flow |
| **Anyone quoting a token's value** | the pair, and the drift between them | presenting a token price as the asset's value |

## What actually exists today

| | |
|---|---|
| `programs/svi-stock-adapter` | written, 19 tests. Verifies Pyth owner, feed id, verification level, publish time, age and confidence, and refuses on each |
| `programs/svi-core` | written. Frozen 320-byte quote account, asserted by a test |
| `tools/svi-keeper` | written, 22 tests. `--feed stock:<SYM>`, with Hermes posting for unsponsored feeds |
| `tools/stock-check` | written. The independent verifier |
| `scripts/plan-deployment.py` | written. Derives every deployment address once |
| deploy runbook | written (`programs/svi-stock-adapter/runbooks/`) |
| **devnet deployment** | **not done yet** |
| **a keeper run across a US market close** | **not done yet** — this is what turns `MARKET_CLOSED` from a description into a screenshot |
| audit | not done, and this says so everywhere |

## The interface already generalises

Before stocks, SVI published the NAV of Hylo's **xSOL** — a value computed by
*subtraction* from a protocol's own vault, with no market price anywhere and
nothing resembling a Pyth feed.

Same core. Same 320-byte account. Same write path. Same rule about refusing
rather than guessing. Only the adapter differs.

On 10 September 2026 that adapter published a real quote on a fork of Solana
mainnet, with Hylo's accounts re-cloned seconds before, and an independent
off-chain reader agreed to the last digit: `$0.060285498`. The record is in
[`docs/validation/2026-09-10-xsol-nav-mainnet.md`](../validation/2026-09-10-xsol-nav-mainnet.md).

Two adapters this different, publishing into one account shape that consumers
read the same way, is the argument for calling this an **interface** rather
than an oracle. An oracle gives you a number. An interface gives every kind of
value a common way to be published, checked and refused.

## Where to go next

| | |
|---|---|
| [`02-product-overview.md`](02-product-overview.md) | the five pieces, in plain language |
| [`03-architecture.md`](03-architecture.md) | how they fit together, with diagrams |
| [`../../README.md`](../../README.md) | the byte layout, the keeper, the verifier |
| [`../hylo-xsol-nav-v1-spec.md`](../hylo-xsol-nav-v1-spec.md) | the frozen feed spec, for the Hylo adapter |

# SVI — One Pager

### Solana Valuation Interface
*On-chain fair value for tokenized stocks.*

---

**THE PROBLEM.** A tokenized stock like **AAPLx** trades on Solana twenty-four
hours a day. The share it represents trades six and a half. For roughly two
thirds of every week the token has a price and the company does not — thin
overnight books, no arbitrage possible, because the thing it tracks is closed.

Nothing on-chain says so. A lending market reading the token's 3am price reads
a number with no indication that it is a guess made in an empty room, and
liquidates against it anyway.

---

**THE FIX.** For each stock, one Solana transaction reads two Pyth feeds and
writes two accounts anyone can read:

- **fair value** — what the real share is worth (`Equity.US.AAPL/USD`)
- **market price** — what the token trades at (`Crypto.AAPLX/USD`)

Both written in the same instruction, so the pair can never be half-updated.
Each carries Pyth's confidence interval as explicit bounds, the slot it
expires at, and flags: `MARKET_CLOSED`, `REFERENCE_STALE`, `TOKEN_FEED_STALE`,
`DEVIATION_HIGH`.

---

**WHY THE FLAGS ARE THE PRODUCT.** The value is the easy part. What makes this
worth deploying is that the account says *what kind of moment it was taken
in*. `MARKET_CLOSED` is not an error — an equity feed is supposed to be hours
old overnight. It is information a consumer can act on: widen the margin, or
refuse the position.

Three thresholds per feed, all on-chain where a third party can read them:
15 minutes flags, 4 days doubts, 8 days **refuses**. Past the refusal the
adapter publishes nothing and the previous quote expires. *Fail stale, never
fail wrong* — an expired quote stops a consumer; a plausible wrong one does
not.

---

**WHY YOU CAN BELIEVE IT.** No API. The math runs in a Solana program, the
inputs are Pyth accounts, the output is an account you read directly.

Cranking is permissionless and the keeper holds no privileged key — `svi-core`
accepts the write only because the *adapter program's own PDA* signed the CPI.
The 32-byte Pyth feed id inside each price account is re-checked on-chain every
refresh, so nobody can pass a real, fully verified price for a cheaper asset
and have it published as Apple's.

And `tools/stock-check` will re-run the whole thing against Pyth using none of
SVI's code.

---

**IT ALREADY GENERALISES.** Before stocks, SVI published the NAV of Hylo's
xSOL — a value computed by *subtraction* from a protocol's vault, with no
market price anywhere. Same core, same 320-byte account, same write path;
only the adapter differs. On 10 Sep 2026 it published a real quote on a
mainnet fork and an independent reader agreed to the last digit:
`$0.060285498`.

An oracle gives you a number. An **interface** gives every kind of value a
common way to be published, checked and refused.

---

**STATUS.** Adapter, core, keeper, verifier, deploy runbook and dashboard are
written and tested — 91 Rust tests, plus the JS and Python suites, all green.
The devnet deployment and a keeper run spanning a US market close are the next
step. Unaudited, and it says so everywhere.

`github.com/princedotrs/solana-valuation-interface`

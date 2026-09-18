# SVI — Product Overview

**Solana Valuation Interface** · plain-language explainer · September 2026

*If you read one document, read this one. It explains what SVI is without
assuming you've read the spec.*

---

> **Scope note.** This document explains SVI's five pieces using Hylo's xSOL
> as its running example, because a value computed by subtraction from a
> protocol vault is the hardest case and makes the design rationale clearest.
> SVI's first adapter is now the **tokenized-stock** adapter; for what SVI is
> *for*, read [00-stocklana-overview.md](00-stocklana-overview.md) first. The
> machinery described below is the same machinery the stock adapter uses.


## The one-sentence version

**The value of a protocol asset is calculated by a program on-chain and
written to a public account that anyone can read — so nobody ever has to
trust an off-chain API doing hidden math.**

Everything else is supporting machinery.

---

## Start with one example

Take **xSOL**, Hylo's leveraged SOL token.

Its fair value isn't set by trading. It's *defined by a formula*: take all the
SOL-denominated value locked in Hylo's pools, subtract what's owed to hyUSD
holders, divide by how many xSOL exist.

```
xSOL NAV  =  (SOL TVL − vUSD supply) / xSOL supply
```

Every one of those numbers lives in an on-chain account **right now**. The
problem is not that the data is missing. The problem is that nobody has
packaged that math into a form that is both trustworthy and readable.

So today the value gets produced like this: a server reads Hylo's accounts over
RPC, runs the formula in private code, and publishes the answer through an API.
The answer arrives having lost the one property that made it trustworthy — the
ability for anyone else to check it.

---

## The five pieces

### 1. The Adapter — the calculator

A program, one per protocol. The Hylo adapter knows Hylo's formula and which
accounts hold the inputs: Hylo's pool accounts, the stake pool accounts, Pyth's
SOL/USD feed.

Its only job: read those accounts, **verify they're the real ones** (right
owner, right PDA, right mint, right program), do the math, and produce a number
with a safety range around it.

Note what it does *not* do: it doesn't call Hylo. Solana programs have no view
functions — but they don't need any, because account data is public bytes. The
adapter is handed the accounts and deserializes them itself. Reading accounts
within one transaction is atomic, so everything it reads is from the same slot.

**Consequence: Hylo never has to change a line of code, and never has to know
we exist.** (We're asking anyway — see the proposal doc for why.)

### 2. The Core — the notice board

A tiny, deliberately dumb program that owns the official "price notes." It
knows nothing about Hylo, or Meteora, or anything else. Its only rules:

- A new note is accepted only if a **registered adapter's PDA signed it**.
- Each note must be **newer** than the last.
- Every note is stamped with when the data was observed and when it expires.
- The value must be internally coherent: `lower ≤ value ≤ upper`.

Dumb on purpose. Dumb is auditable — and one day it can be frozen forever.

### 3. The Quote account — the note itself

One account per feed (one for xSOL NAV, one for hyUSD backing, and so on).
320 fixed bytes containing the value, its lower and upper bounds, the slot it
was observed at, when it expires, a monotonic sequence number, status flags,
and a hash identifying exactly which formula version produced it.

**This account is the product.** Everything else exists to keep it honest.

### 4. The Keeper — the person who rings the bell

Prices don't refresh themselves; some transaction has to trigger the
calculation. That's a small bot, sending a refresh transaction every ~60
seconds, costing a fraction of a cent each time.

**The subtlety that matters:** anyone can send this transaction — it's
permissionless, there's no trusted crank operator — but **nobody can influence
what value gets written**, because the value is computed by the adapter from
verified accounts, and only the adapter's PDA can authorize the write.

The keeper is just paying for gas. If our bot dies, anyone else's bot can crank
it. If nobody cranks it, the quote goes stale and its expiry slot makes that
visible. *It fails loud; it never lies.*

### 5. The Consumers — the readers

Pyth, lending protocols, wallets, dashboards. They read the quote account like
any other account — a few hundred compute units.

---

## The mental model: a company's accounting

| The thing | The analogy |
|---|---|
| Hylo's raw accounts | **The receipts** — all the truth is there, but scattered and unreadable |
| The adapter | **The robot accountant** — whose procedure is published (the methodology hash), and who refuses to work with unverified receipts |
| The quote account | **The audited statement posted on the wall** — one page, standard format, dated, with a "valid until" stamp |
| The core program | **The rule that only the accountant may post to the wall**, and each statement must be newer than the last |
| The mirror API | **A photocopy of the statement** — useful for people who won't come to the office, and verifiable against the original by anyone |
| The observer | **A second, independent accountant** who redoes the math from the same receipts. If his number ever differs, alarms go off and photocopying stops |

**The whole design reduces to one principle:** there is no point in the
pipeline where a human or a hackable server *chooses* the number. The number is
a pure function of on-chain state, computed by public code, and everything
downstream only transports it.

---

## How a refresh actually works

Every ~60 seconds, one transaction:

1. The keeper calls the adapter's refresh instruction, passing in Hylo's
   accounts, the stake pool accounts, Pyth's SOL/USD account, and the quote
   account.
2. The adapter **verifies every account** — owner, PDA derivation, mint,
   program ID. Anything off → abort, no write. *(This is what stops someone
   passing in a fake pool account stuffed with tokens.)*
3. It computes NAV using `hylo-core` — Hylo's own math library, pinned to an
   exact commit — with all-integer arithmetic. Pyth's confidence interval
   becomes the lower and upper bounds.
4. It CPIs into the core: "here's the new xSOL quote, signed by my PDA."
5. The core checks the rules and overwrites the quote account. Done.

If **anything** is wrong — a stale oracle, an epoch-boundary cache that hasn't
been cranked, an upgraded target program — the transaction aborts and writes
nothing. The old quote ages past `valid_until_slot` and consumers see it as
expired.

> **Fail stale, never fail wrong.**

---

## How a value gets used

**Mode A — read the note (99% of usage).** A lending market passes the quote
account into its own instruction, checks that `valid_until_slot` is still in
the future and the status flags are healthy, and uses the lower bound for
collateral. One account read.

**Mode B — call the calculator live (high-stakes only).** A liquidator worried
that a 60-second-old quote is too stale can include the adapter's computation
in their own transaction, so the value is computed from that exact slot's
state, atomically. More accounts, more compute, zero staleness. Most
integrators never need this.

**Mode C — the mirror API.** For off-chain consumers like Pyth: fetch the
account through two independent RPCs, require they agree byte-for-byte, decode,
serve as JSON with full provenance. **The API performs no arithmetic.** It's a
photocopier — it can't lie about the price, because anyone can compare its
output against the account.

---

## Why the value types matter

A consumer asking for NAV must never silently receive a market price. So the
kind of value is a first-class field, validated on-chain:

| Type | Meaning |
|---|---|
| `PROTOCOL_NAV` | The asset's share of protocol collateral, per the protocol's own math |
| `REDEMPTION` | What you actually receive redeeming right now, after fees |
| `EXCHANGE_RATE` | e.g. sHYUSD → hyUSD |
| `BACKING_NAV` | Stablecoin backing value |
| `MARKET_SPOT` | A DEX price. **Never** use for collateral |
| `MARKET_TWAP` | Time-weighted market price |

This one field is the difference between the Moonwell incident — where a
config change quietly redefined what a feed meant, and liquidation bots
produced $1.8M of bad debt — happening to an SVI consumer or not.

---

## Adding a new protocol

Supporting, say, Meteora DAMM v2 LP tokens means: write one new adapter that
knows DAMM v2's account layout, register its PDA in the core as the authority
for those feeds, point a keeper at it, publish a methodology spec.

**Meteora never hears from us.** The core, quote format, mirror API, observer
and SDKs are all shared. Each new protocol is only a new calculator plugged
into existing rails.

---

## What's built today

| Component | State |
|---|---|
| `svi-math` — exact integer math, mandatory rounding, no floats, property-tested | ✅ **Done** |
| `svi-core` — descriptor + 320-byte quote, adapter-PDA authorization, monotonic sequence, freshness rules | ✅ **Done** |
| `svi-mock-adapter` — proves the CPI + authority path end to end | ✅ **Done** |
| `hylo-xsol-nav-v1` methodology spec | 🟡 **Draft v0.9** — 5 open questions for Hylo |
| `svi-hylo-adapter` — the real calculator | 🔒 **Scaffolded**, blocked on those 5 answers |
| Keeper, mirror API, observer | ⬜ Phase 2 |

**The critical path is not engineering capacity. It's five answers from Hylo.**

---

## Where to go next

| If you want | Read |
|---|---|
| Is this worth building? Who else is doing it? | [`01-market-analysis.md`](01-market-analysis.md) |
| How does it actually work? | [`03-architecture.md`](03-architecture.md) |
| What exactly gets built, in what order? | [`04-user-stories.md`](04-user-stories.md) |
| How does this reach adoption? | [`05-gtm.md`](05-gtm.md) |
| What are we asking Hylo for? | [`06-hylo-proposal.md`](06-hylo-proposal.md) |
| The exact math and accounts | [`../hylo-xsol-nav-v1-spec.md`](../hylo-xsol-nav-v1-spec.md) |

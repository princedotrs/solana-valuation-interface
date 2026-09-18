# The videos — plan, scripts, and shot list

Two cuts from one recording session:

| Cut | Length | Where | What it has to do |
|---|---|---|---|
| **Pitch** | 3 min | Colosseum / Stocklana submission, Twitter | Make someone who prices collateral for a living stop scrolling |
| **Technical** | 5 min | The submission's second link, DMs to protocol teams | Show the mechanism working and be honest about what is not done |

Everything on screen is real: the keeper, the watcher, the dashboard, the test
output. No mock-ups, no "coming soon" slides inside the demo.

---

## The one claim

> **A tokenized stock trades all night. The company doesn't. SVI puts the real
> value on-chain, in an account anyone can read, that tells you when the
> market is shut.**

Say it at the start, prove it in the middle, repeat it at the end.

### Three things never to say

- **"Fresher than Pyth."** Pyth publishes the input; SVI publishes the output.
  A quote can never be fresher than the price it came from. Saying otherwise
  will get corrected in the replies, and deservedly.
- **"Live on mainnet"** — until it is. Say "devnet", plainly, every time.
- **"Audited"**, or anything that implies it. It is not.

### The one thing to say that nobody else will

*The failure mode we designed for is being wrong, not being late.* Most of the
code is refusals. That is the whole pitch to anyone who has been liquidated by
a bad oracle print.

---

## Recording checklist

Before you hit record, have all of this already running:

- [ ] Devnet deployed, `deployments.json` written, at least one crank landed
- [ ] Terminal 1: `svi-keeper crank --feed stock:AAPL --interval 15`
- [ ] Terminal 2: `svi-keeper watch --feed stock:AAPL`
- [ ] Terminal 3: clear, for `stock-check`
- [ ] Browser tab 1: the dashboard, already loaded
- [ ] Browser tab 2: Solana Explorer on the fair-value quote account
- [ ] Font size up. A 1080p recording of an 11px terminal is unreadable on a phone.
- [ ] **Record during or just after a US market close if you possibly can.**
      `MARKET_CLOSED` appearing live is worth more than any explanation of it.

If you cannot record across a real close, say so on screen — "this flag was
captured at 16:15 ET on the 18th" over the validation record — rather than
implying it is happening live.

---

# Cut 1 — the pitch (3 minutes)

### 0:00–0:20 · The hook

> *[screen: a tokenized stock's chart, overnight]*
>
> This is AAPLx. A tokenized Apple share, trading on Solana at three in the
> morning.
>
> Apple is closed. It has been closed for eleven hours. There is no market
> anywhere that will arbitrage this price back to anything real, because the
> thing it tracks is shut.
>
> And nothing on-chain says so.

### 0:20–0:50 · The consequence

> *[screen: a lending position, then a liquidation]*
>
> A lending market reads that price. It doesn't know the difference between a
> price and a guess made in an empty room — it just sees a number. So it
> liquidates someone against it.
>
> By nine-thirty, when Apple opens, the price is back. The liquidation isn't.
>
> Tokenized equities are the fastest growing thing on Solana right now. Every
> one of them has this hole in it.

### 0:50–1:40 · The fix

> *[screen: the dashboard, live]*
>
> This is SVI. For each stock it publishes two numbers on-chain, written in
> the same transaction.
>
> *[point]* What the real share is worth. *[point]* What the token is actually
> trading at. And the gap between them.
>
> Both come from Pyth — the equity feed and the crypto feed, read together in
> one instruction, so the drift is measured at a single instant.
>
> *[point at the flags]* And this is the part that matters. `MARKET_CLOSED`.
> The account is telling you the exchange is shut. Not an error — an equity
> price is *supposed* to be hours old overnight. It's information. Widen your
> margin, or don't take the position.
>
> *[point]* `DEVIATION_HIGH`. The token is more than two percent from the real
> share. That's a dislocation, on-chain, that anyone can read.

### 1:40–2:20 · Why you can believe it

> *[screen: terminal, `stock-check AAPL`]*
>
> There's no API here. The math runs in a Solana program, the inputs are Pyth
> accounts, the output is an account you read directly — about two hundred
> compute units, no CPI.
>
> And you don't have to take my word for any of it. This tool re-reads the
> published accounts, asks Pyth independently, and recomputes everything.
>
> *[output lands]*
>
> It shares no code with the adapter. Written separately, on purpose. Two
> implementations agreeing is evidence. One agreeing with itself isn't.

### 2:20–2:45 · It generalises

> *[screen: the Hylo section of the site]*
>
> Before stocks, the same core published the NAV of a leveraged token —
> computed by subtraction from a protocol's vault, nothing like a Pyth feed.
> Same account shape, same write path, same rules. Only the adapter changed.
>
> That's why it's an interface and not an oracle. An oracle gives you a
> number. An interface gives every kind of value a common way to be published,
> checked, and refused.

### 2:45–3:00 · The close

> On devnet today. Unaudited, and the repo says so on every page. The adapter,
> the keeper, the verifier and the dashboard are all there, all tested.
>
> If you price collateral against a tokenized stock, this is the number you
> should be reading at three in the morning.

---

# Cut 2 — the technical walkthrough (5 minutes)

### 0:00–0:30 · What you're about to see

> Four things: the adapter refusing, the two feeds landing in one transaction,
> the flags appearing, and an independent tool agreeing.
>
> Everything is devnet. Nothing is mocked.

### 0:30–1:30 · The refusals

> *[screen: `programs/svi-stock-adapter/src/pyth.rs`]*
>
> Most of this program is refusals, and that's deliberate.
>
> *[scroll]* The account has to be owned by the Pyth receiver. It has to
> deserialize. Its verification level has to meet the minimum this symbol's
> config demands. Its feed id has to equal the one in the config — *[pause]*
> that's the important one. A keeper picks which accounts go into a
> transaction. Without this check, anyone could pass a real, fully-verified
> Pyth price for a different, cheaper asset and have it published as Apple's.
>
> Then: publish time positive, not from the future, not older than the hard
> limit, converts to a positive value, confidence not wider than five percent.
>
> Any one of those fails and the transaction reverts. Nothing gets published,
> and the previous quote just expires. *Fail stale, never fail wrong.*

### 1:30–2:30 · Two windows per feed

> *[screen: `state.rs`, the staleness fields]*
>
> Here's the design decision the whole thing turns on.
>
> "Too old to use" and "old enough to mention" are different questions. If I'd
> collapsed them, the product would be impossible — an equity feed is supposed
> to be hours old overnight, so refusing to publish when the reference is
> stale would take the feed dark at exactly the moment you most need to be
> told the market is shut.
>
> So there are three thresholds. Fifteen minutes raises `MARKET_CLOSED` — a
> gap that long during a session would itself be a fault, and a quarter hour
> after the bell is exactly when you want to start saying it. Four days adds
> `REFERENCE_STALE`, above any long weekend. Eight days refuses outright.
>
> All three are in the symbol's on-chain config. A third party can read the
> rules that produced a quote instead of trusting my description of them.

### 2:30–3:30 · One transaction, two quotes

> *[screen: terminal 1, the keeper cranking; terminal 2, the watcher]*
>
> The keeper is permissionless. It holds no privileged key — `svi-core`
> accepts the write only because the adapter program's own PDA signed the CPI.
> Anyone can run this and nobody running it can change the number.
>
> *[a crank lands]*
>
> Both quotes, same instruction, same `observed_slot`. The pair can never be
> half-updated — there's no moment where a fresh market price sits against a
> stale fair value, because there's no moment where one exists without the
> other.
>
> *[screen: Explorer on the quote account]*
>
> And there it is on-chain. Three hundred and twenty bytes, frozen layout,
> asserted by a test in `svi-core`. Anyone reads it with four lines and about
> two hundred compute units.

### 3:30–4:15 · Bringing our own prices

> *[screen: `--post-updates` running]*
>
> One honest complication. Pyth sponsors price accounts for some feeds — kept
> fresh, fully verified. Equity feeds on devnet generally aren't sponsored, so
> there's nothing to read.
>
> So the keeper can bring the price itself: fetch a signed update from Hermes,
> post it, refresh against it. But posting in a single transaction only checks
> a subset of guardian signatures, which gives you a *partial* verification
> instead of a full one.
>
> That's a real weakening and I didn't want to hide it. So it's a per-symbol
> field in the on-chain config. A feed running at the weaker level is visibly
> running at the weaker level — not described as safe in a keeper config
> nobody else can see.

### 4:15–4:50 · The second opinion

> *[screen: terminal 3, `stock-check AAPL`]*
>
> This depends on none of SVI's crates. The scaling arithmetic is written out
> again from Pyth's exponent convention, and its tests pin sixteen vectors
> against `svi-math`, which is the code the adapter actually runs.
>
> It reads the accounts, asks Hermes independently, and complains if a quote
> is outside its own bounds, mistyped, or if the pair wasn't written at the
> same slot.

### 4:50–5:00 · What isn't done

> Devnet, not mainnet. Unaudited. `methodology_hash` is still zero because the
> spec isn't frozen. No lending market is consuming it yet — that's next, and
> it's the thing that would make this real.
>
> All of that is written down in the repo, in the same words.

---

## Shot list

| # | Shot | Source | Note |
|---|---|---|---|
| 1 | AAPLx overnight chart | any chart tool, or the dashboard's own drift number | must show a real gap |
| 2 | Dashboard, live, flags visible | `site/index.html` on devnet | the money shot; get `MARKET_CLOSED` if at all possible |
| 3 | Keeper cranking | terminal 1 | let two cycles land, don't cut |
| 4 | Watcher | terminal 2 | shows the quote changing under it |
| 5 | `stock-check` output | terminal 3 | the verdict block, uncut |
| 6 | `pyth.rs` refusal list | editor | scroll slowly, the comments carry it |
| 7 | `state.rs` staleness fields | editor | the three thresholds |
| 8 | Explorer on the quote account | browser | proves it is a real account |
| 9 | Hylo validation record | `docs/validation/` | the generalisation claim |
| 10 | Test suite, green | terminal | one take, no cuts, shows the count |

## Recording notes

- **One take per terminal shot.** A cut in the middle of a crank looks like
  something was hidden.
- **Don't speed anything up.** If a transaction takes four seconds, four
  seconds is the truth about how fast this is.
- **Show a failure on purpose** in the technical cut if you have time — point
  the keeper at a wrong price account and let `PythFeedMismatch` come back.
  The refusals are the pitch; showing one land is stronger than describing it.

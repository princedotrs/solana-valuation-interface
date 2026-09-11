# The pitch video — plan, script, and shot list

One recording session yields two cuts:

| Cut | Length | Where | What it has to do |
|---|---|---|---|
| **Short** | 75–90 s | Twitter/X, the top of the thread | Make one person who prices collateral for a living stop scrolling |
| **Full** | 3½–4 min | Colosseum submission, Superteam grant, DMs to lenders | Show the whole mechanism working and be honest about what is not done |

Everything on screen is real: the keeper, the watcher, the test output, the
page. No mock-ups, no "coming soon" slides in the demo section.

## The one claim

> **Nobody publishes this number. Now it is on-chain, computed by code anyone
> can run, in an account anyone can read, with bounds and a staleness horizon.**

Say it at the start, prove it in the middle, repeat it at the end.

Two things never to say, because they are false and the replies will find it:

- *"Fresher than Pyth."* Pyth publishes SOL/USD, the **input**. xSOL NAV is
  computed from it, so our quote is always ≤ 10 s behind Pyth by Hylo's own
  rule. What we publish is the **output**, which no oracle carries.
- *"Live on mainnet"* — until it is. Until then, say "a fork of mainnet state,
  same bytes, local validator", and say it early. The deploy is ~3.3 SOL away.

## Screen setup

Three terminal panes in one window (tmux or iTerm split), dark theme, font
18–20 pt so it reads on a phone. Record at 1080p or 1440p, 60 fps — the
watcher redraws every second and the tick is part of the point.

```
┌────────────────────────────┬────────────────────────────┐
│ [1] svi-keeper watch       │ [2] svi-keeper watch --pyth│
│     our quote, 1 s redraw  │     the input, its own age │
├────────────────────────────┴────────────────────────────┤
│ [3] svi-keeper crank --interval 5 --surfnet             │
│     one line per publish, refusals decoded              │
└─────────────────────────────────────────────────────────┘
```

Commands, in order, before you press record:

```bash
# fresh fork — do not reuse an old one
surfpool start

# in the repo
export SVI_MAINNET_RPC='<your helius url>'     # the tools never print it
K=tools/svi-keeper/target/debug/svi-keeper
$K crank --interval 5 --surfnet                # pane 3, first: deploys nothing,
                                               # so run the test once beforehand
                                               # if the programs are not there
$K watch                                       # pane 1
$K watch --pyth                                # pane 2
```

If `crank` says a program is not deployed, run
`cargo test --test surfnet -- --nocapture` once in `programs/svi-hylo-adapter`
— it deploys both programs and makes the first publication. Let the keeper run
for a minute before recording so `sequence` is past 1 and the log has a few
lines.

B-roll to capture separately, each 10–15 s, cursor hidden:

1. `docs/product/08-how-svi-values-xsol.html` — slow scroll from the title
   through the dependency-chain figure and the full architecture figure.
2. The test run: `cargo test --test surfnet -- --nocapture`, from
   `deployed …` to `MATCH`. This is the receipt; record it clean.
3. Deck slides 2 (receipt), 4 (trust chain), 11 (fail stale) as stills.
4. Plish's post, as a screenshot with the date visible.

## Full cut — script with timings

Read it as one person explaining something they built, not as a voiceover.
Short sentences. Pause on the numbers.

**0:00 – 0:15 · Cold open** — *pane 1 full-screen, the watcher ticking*

> This is the value of one xSOL, right now, on-chain.
> It's $0.0603. Bounds 4.9 basis points wide. Seven slots old.
> Until this week, this number did not exist anywhere on Solana.

**0:15 – 0:45 · The problem** — *Plish's post, then slide 4*

> xSOL is Hylo's leveraged SOL token. Its value isn't a market price —
> it's a formula over Hylo's own books. Nobody publishes the result.
> Hylo's website computes it in your browser.
>
> So when Pyth wanted an xSOL feed, they asked Hylo to read their own
> protocol over RPC, do the math on a server, and hand a number to an API.
> Hylo's founder called that "unimaginable security holes". He's right —
> that's five trusted hops, and a lender can check none of them.
> Four lenders have already lost money to exactly this failure class.

**0:45 – 1:15 · What the number is** — *page section "What xSOL is", the subtraction*

> Here's the formula. Hylo holds about 120,000 SOL. At $212, that's
> $25 million. It owes $15 million to hyUSD holders. What's left — $10
> million — belongs to 166 million xSOL. Ten million divided by 166 million
> is six cents.
>
> Two inputs: the SOL price, from Pyth, and Hylo's books, from Hylo's
> accounts. Both public. So the output can be computed on-chain, by a
> program, from bytes anyone can verify. That's what SVI does.

**1:15 – 2:15 · The demo** — *three panes*

> Bottom pane is the keeper. Every five seconds it sends one transaction.
> Anyone can run this — it holds no key that matters. It pays the fee, and
> that's all it can do.
>
> The adapter reads Hylo's state, the xSOL mint, and Pyth SOL/USD. It checks
> every account is the genuine one. It runs Hylo's own math library — the
> same code Hylo's program uses, pinned to one commit — and it publishes.
>
> Top left: the result. Value, bounds, the slot it was observed at, when it
> expires, and flags. `BUY_ZONE` means Hylo's collateral ratio is in a
> rebalance zone right now. The adapter says so instead of hiding it.
>
> Top right: the input. Pyth's SOL/USD, with its own age. Watch SOL move —
> *(pause for a tick)* — and xSOL moves about two and a half times as much.
> That's the leverage, live.
>
> Now watch what happens when I stop the keeper. *(Ctrl-C pane 3.)*
> The quote doesn't change. It ages. In five minutes it says STALE — do not
> use. It never guesses. The rule is: fail stale, never fail wrong.
> *(Restart the keeper.)*

**2:15 – 2:45 · The receipt** — *B-roll 2, the test output*

> This is the test that proves it. It deploys both programs, re-clones
> Hylo's accounts from mainnet, publishes, reads the quote back by byte
> offset the way a lender would, and then recomputes the value off-chain
> with Hylo's library over the same bytes.
>
> On-chain: 0.060285498. Off-chain: 0.060285498. To the last digit.

**2:45 – 3:15 · How a lender uses it** — *page section "What a reader does with it"*

> The integration is one account read, about two hundred compute units.
> Four fields matter: is it still valid, are the flags clear, what's the
> lower bound. A hundred thousand xSOL deposited is six thousand dollars of
> collateral at the lower bound. When the quote expires, the market refuses
> new borrows. It never has to decide whether a stale number is probably
> fine.

**3:15 – 3:40 · What's not done** — *slide 13*

> Three honest caveats. This ran on a fork of mainnet — same bytes, local
> validator; mainnet deployment is next and costs about three SOL. It
> depends on Pyth for the SOL price and always will; our quote can't be
> fresher than its input. And it's not audited, so no market should
> underwrite against it yet. That's what the grant is for.

**3:40 – 4:00 · Ask and close** — *slide 17, then the repo*

> If you price collateral: read one account. If you're Superteam: an audit,
> a mainnet deployment, and three months of keepers. If you're Hylo: fifteen
> minutes reading the spec and telling me if I misread your math — the
> adapter runs either way.
>
> Everything is in the repo: programs, tests, keeper, the validation record
> with the transaction signature. Fail stale, never fail wrong.

## Short cut — 75–90 s

Cold open (0:00–0:15) → the formula in one breath (15 s) → the demo with the
STALE beat (35 s) → the receipt line "on-chain 0.060285498, off-chain
0.060285498" (10 s) → one caveat: "on a fork; mainnet next; not audited" (7 s)
→ repo (5 s). Captions burned in; most people watch muted.

## Thread copy (post with the short cut)

1. The value of one xSOL, computed on-chain, in a public account. Bounds,
   staleness, flags. Nobody published this number before. Here's it ticking. 🧵
2. xSOL is a formula, not a price: (Hylo's SOL × SOL/USD − hyUSD debt) ÷
   supply. Pyth has the input. Nobody had the output. SVI computes it on-chain
   with Hylo's own math library and writes 320 bytes anyone can read.
3. The receipt: on-chain $0.060285498, independent off-chain recomputation
   $0.060285498. Same to the last digit. Tx sig + validation record in the
   repo.
4. Anyone can run the keeper. It can't influence the value — only the
   adapter's own PDA can write, after every check passes. Stop cranking and
   the quote goes STALE, visibly. It never guesses. Fail stale, never fail
   wrong.
5. Honest bits: this is a mainnet *fork* (same bytes, local validator);
   mainnet is ~3 SOL away. Depends on Pyth for SOL/USD, always will. Not
   audited — no real collateral until it is. Repo + spec:
   github.com/princedotrs/solana-valuation-interface

## Checklist before publishing

- [ ] No RPC URL visible in any frame (the tools redact; check your shell prompt and `env`)
- [ ] The word "fork" said before minute one
- [ ] The phrase "fresher than Pyth" appears nowhere
- [ ] Sequence number > 1 in the opening shot
- [ ] Captions on the short cut
- [ ] Helius key rotated (it has been in a chat transcript)

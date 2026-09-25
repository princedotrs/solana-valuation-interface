# Filming the two videos — shot list, narration, timings

Everything below runs on the machine you have, against the fork you already
ran. No Hermes key, no devnet deployment, no waiting on anyone.

Read `10-submission.md` first for who each video is for. This document is the
one you have open while recording.

---

## Before you press record

**Rehearse every command once.** Half the commands here depend on local state
that has bitten us already: a stale `deployments.json`, a `declare_id!` that
moved, a fork whose Pyth accounts aged out. If a command fails on camera you
lose the take and the confidence.

**The fork ages.** Hylo's oracle window is ten seconds and a Surfnet's clock
drifts from the slot it snapshotted, so a fork left running for an hour will
fail where a fresh one succeeds. Restart `surfpool` immediately before the
technical video and do the Hylo run early in the take.

Setup that shows up on screen:

- Terminal at **16-18pt**, full screen, light background if your dashboard is
  light. A 12pt terminal is unreadable at 720p on a phone.
- **Notifications off.** System, Slack, everything.
- Prompt shortened to something anonymous — no `/Volumes/Crucial X9/...`, no
  hostname. `PS1='$ '` for the recording.
- Record at **1080p**. Judges scrub on laptops; 720p makes hex unreadable.
- One terminal, one browser tab. Splitting attention costs more than it shows.

**Do not** speed up terminal output in the edit. A judge who has watched forty
demos knows what a real compile looks like. Cut to the result instead.

---

## Pitch video — 3:00

Audience: someone who prices collateral for a living. They decide in fifteen
seconds whether to keep watching.

### The claim, said at the start and repeated at the end

> Some assets have no price. Not a stale one — none. xSOL is worth whatever
> Hylo's reserves say it is worth, and nobody publishes that number. We do, in
> an account anyone can read, and we refuse to publish when we cannot prove it.

### 0:00 – 0:20 · The number

**Screen:** the tail of the surfnet run, `PUBLISHED QUOTE` block visible.

> "This is what one xSOL is worth right now: six point nine seven cents. It is
> not quoted on any exchange, and no oracle publishes it. Until this account
> existed, the only way to know it was to write the maths yourself."

### 0:20 – 0:50 · Where it comes from

**Screen:** the three Hylo account addresses, then the quote block again.

> "xSOL is the leveraged tranche of Hylo's stablecoin. Its value is the
> collateral, times the SOL price, minus the hyUSD debt, divided by supply.
> Four numbers from Hylo's own books. Our program reads those accounts
> on-chain, does that arithmetic on-chain using Hylo's own published library,
> and writes the answer to an account. No server. No API key. Nothing to trust
> about us."

### 0:50 – 1:25 · Why anyone should care

**Screen:** `python3 scripts/check-pyth.py --rpc https://api.devnet.solana.com`

> "Here is the problem this sits next to. These are price feeds people assume
> are live. Six days stale. Thirty-four days. Seventy-eight on devnet, and two
> of these accounts do not exist at all. Nobody noticed, because nothing
> refuses — an oracle serves what it has and leaves the judgement to you."

Let the red scroll. Do not narrate over it.

### 1:25 – 2:00 · The refusal

**Screen:** the keeper refusing with `PythTooOld (6006)`, then the decoded line.

> "Ours refuses. Wrong owner, wrong asset, partially verified, published in the
> future, too old, confidence too wide — six checks, and any one of them stops
> the write. The quote goes stale visibly instead of going wrong quietly. Fail
> stale, never fail wrong."

### 2:00 – 2:30 · That it generalises

**Screen:** the stock pair — fair value against market price — in the deck or
the dashboard.

> "The same account format, a different asset. A tokenized Tesla share against
> the real one. Pyth publishes both legs; nobody publishes the gap between
> them. That gap is what a lender needs and what nobody is watching."

### 2:30 – 3:00 · Check it yourself, and the limits

**Screen:** the dashboard, then the repository.

> "One format, any asset, written only by code that can prove its inputs. The
> addresses are on the page — run the same `getMultipleAccounts` from your own
> terminal and you will get the same bytes. It is open source, anyone can crank
> it, and there is no token. It runs on a mainnet fork today, it is not
> audited, and nothing should be collateralised against it until it is."

Ending on the limits is not weakness. It is the only part of the video a
sceptical judge was not expecting.

---

## Technical video — 5:00

Audience: deciding whether this is real. They will forgive unfinished. They
will not forgive hidden.

### 0:00 – 0:40 · Run it

**Command:**

```
cargo test --manifest-path programs/svi-hylo-adapter/programs/svi-hylo-adapter/Cargo.toml \
    --test surfnet -- --nocapture
```

> "One command. It deploys both programs, re-clones Hylo's three accounts from
> mainnet, pins the clock, refreshes, and reads the quote back. Watch the
> bottom."

### 0:40 – 1:20 · What the clock pinning is, and is not

**Screen:** the `clock (live)` and `clock (pinned)` lines.

> "Hylo will not price against an oracle update more than ten seconds old, or
> more than twenty-five slots away. A local fork ticks at a fixed four hundred
> milliseconds while its clock follows real time, so after a few minutes those
> two constraints cannot both hold. So the test sets the clock to the slot and
> timestamp of the snapshot it just took. No Hylo byte is altered. No Pyth byte
> is altered. This is exactly what a fork taken at that slot would have seen —
> the Pyth update is one second and one slot old, well inside Hylo's own
> tolerance."

Say this clearly. It is the part a sceptical engineer is most suspicious of,
and the honest answer is completely defensible.

### 1:20 – 2:00 · The account format

**Screen:** `svi-core`'s descriptor and quote structs; then the layout test.

> "One account shape for every asset: value, a lower and upper bound, the slot
> it was observed at, the slot it stops being valid, a sequence number, and
> flags. Three hundred and twenty bytes, asserted by a test, because a consumer
> reads it by byte offset. A write is accepted only from the adapter PDA named
> in the descriptor — so even a compromised keeper cannot write a value, it can
> only pay for one to be computed."

### 2:00 – 3:00 · Two refusals, live

**First — the natural one.** With the sponsored feeds as stale as they are:

```
cargo run --manifest-path tools/svi-keeper/Cargo.toml -- \
    crank --feed stock:AAPL --rpc http://127.0.0.1:8899
```

> "This is the stock adapter reading the price accounts Pyth sponsors. Too old.
> Six thousand and six. It will not publish."

**Then — the deliberate one.** Swap `equity_price` and `token_price` for one
symbol in `deployments.json` and re-run the same command.

> "Now I hand it a real, fully verified, current Pyth price — for the wrong
> asset. Six thousand and three. The program re-read the feed id inside the
> account and compared it to the one in its config. This is the check that
> stops a malicious keeper, and it fires before the staleness check, which is
> why you see it even with these feeds."

Restore `deployments.json` on camera. It costs four seconds and shows the
swap was the only change.

### 3:00 – 3:45 · Independent agreement

**Screen:** the `MATCH` line from the surfnet run.

> "The last line of that test is an off-chain reader computing the same value.
> It calls Hylo's library directly — different account decoding, different
> scaling, different rounding path, nothing shared but the maths. It agrees to
> the ninth decimal. That is evidence the adapter's own plumbing introduces no
> drift."

Then the dashboard, and its expired state:

> "And when there is no current quote, the page withholds the number rather
> than showing it next to a warning. Same discipline as the program."

### 3:45 – 4:25 · The measurement

**Screen:** `check-pyth.py` against devnet and the fork.

> "This tool asks a cluster whether the feeds a deployment needs are actually
> usable. It is how we found that the equity feeds are weeks stale and the
> xStock accounts are missing on devnet. It gives three answers, because they
> need three different responses: usable now, needs a price of your own, or do
> not proceed."

### 4:25 – 5:00 · What is not done

Say all of it. Do not soften.

> "No consumer reads this feed yet. There is no audit, and nothing should be
> collateralised against it until there is. The stock feeds have no price
> source — Pyth's sponsored accounts stopped updating and Hermes now needs a
> key we do not have, so those feeds are blocked, not shipped. The xSOL feed
> needs none of that: it reads Hylo's accounts directly, which is why it works
> today. It runs on a mainnet fork, not on mainnet."

---

## The one thing to get right

A judge who has watched forty oracle demos has seen forty numbers appear on a
screen. None of them has seen a program **decline** to produce one.

Spend the time on the refusals. They are thirty seconds each and they are the
only part of either video that is genuinely hard to fake.

---

## Rehearsal checklist

- [ ] `surfpool` restarted within the last ten minutes.
- [ ] The Hylo surfnet test passes, printing a NAV and `MATCH`.
- [ ] `check-pyth.py` against devnet prints the red staleness output.
- [ ] The stock crank refuses with `6006`.
- [ ] The swapped `deployments.json` refuses with `6003`, and is restored.
- [ ] The dashboard loads, and its expired state reads correctly.
- [ ] Terminal ≥16pt, prompt anonymised, notifications off, 1080p.
- [ ] You can say "mainnet fork" without hesitating.

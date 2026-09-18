# The submission — four links, and what each has to prove

The form asks for a repository, a demo URL, a pitch video and a technical
video. Each is a different audience arriving with a different question, and the
fastest way to lose is to answer the same question four times.

| Field | Who opens it | The question they arrive with |
|---|---|---|
| GitHub repository | A judge who codes | Is this real, and did they write it? |
| Demo URL | Everyone, first | What does it do, in ten seconds? |
| Pitch video (3 min) | A judge deciding what to shortlist | Why should this exist? |
| Technical video (5 min) | A judge deciding if it is real | Does the mechanism work, and do they know its limits? |

---

## What changed, and why this document exists

Two findings from 18 September reshape all four links.

**The stock feeds have no price source.** Pyth's sponsored equity accounts are
6-34 days stale on mainnet and 78 days on devnet; two of three xStock accounts
do not exist on devnet at all; and Hermes now answers `401` without a key.
Measured with `scripts/check-pyth.py`, which is in the repository and which a
judge can run.

**The xSOL feed is unaffected.** It reads Hylo's own accounts and computes NAV
from reserves. It needs no Hermes, no sponsored feed, and no key.

So the order flips. **Lead with xSOL.** It is the number that exists nowhere
else, and it works today. Tokenized stocks become the second example — proof
the interface generalises — rather than the headline.

This is not spin. For stocks, Pyth publishes both legs and the adapter's
contribution is the comparison and the refusal; that is a real contribution and
a modest one. For xSOL there is no feed at all. Lead with the strong claim.

---

## 1. GitHub repository

`https://github.com/princedotrs/solana-valuation-interface`

Before submitting, the root `README.md` must let a judge reach a running thing
in under five minutes. Check that it states, near the top:

- what a `Quote` is, in two sentences, with the 320-byte layout named;
- the one command that runs the xSOL end-to-end test;
- the one command that reproduces the staleness finding;
- what is not done: no consumer, no audit, no price source for stocks.

Verified test counts, as of 18 Sep — cite these, not round numbers:

| Component | Tests |
|---|---|
| `svi-core` | 6 unit, plus integration tests needing a BPF build |
| `svi-stock-adapter` | 26 |
| `svi-hylo-adapter` | 17, including a live surfnet run |
| `svi-keeper` | 31 |

---

## 2. Demo URL — the landing page

`site/index.html` is a single file with no backend: the browser reads the quote
accounts over a public RPC and decodes the layout itself. That property is the
demo. Deploy it as a static page (GitHub Pages, Vercel, Netlify — all fine;
there is nothing to build).

```
# GitHub Pages, from the repository root
git subtree push --prefix site origin gh-pages
# or point Pages at /site on the default branch in repository settings
```

The page must carry, above the fold:

1. **One sentence** saying what this is. Not a tagline — a sentence a lender
   would understand.
2. **A live quote**, read in the visitor's own browser, with its age ticking.
3. **The addresses**, so a visitor can run the same `getMultipleAccounts` call
   from their terminal and get the same bytes. This is the credibility move:
   nothing is being taken on trust.
4. **A link to the repository and both videos.**

### The empty state is part of the demo

`RPC` in `site/index.html` currently points at devnet, where nothing is
published. A page that renders an error, or worse a stale last-known value,
loses more than it gains.

Point it at whatever is actually live. If nothing is, the page must say so in
words a visitor understands — *"no quote has been published for this feed since
<time>; this feed is not being cranked"* — and must never show a number it
cannot justify. That is the same discipline as the on-chain refusal, and a
judge who notices it will value it more than a number that is always green.

---

## 3. Pitch video — 3 minutes

The audience is someone who prices collateral for a living. They have watched
forty of these. Make them stop scrolling in the first fifteen seconds.

### The claim

> **Some assets have no price. Not a stale one — none. xSOL is worth whatever
> Hylo's reserves say it is worth, and nobody publishes that number. We do, in
> an account anyone can read, and we refuse to publish when we cannot prove it.**

### Beats

| Time | On screen | What you say |
|---|---|---|
| 0:00-0:20 | The xSOL quote in a terminal, moving | "This number is what one xSOL is worth right now. It is not on any exchange and no oracle publishes it. It comes from Hylo's own reserves, and until now you could only get it by writing the maths yourself." |
| 0:20-0:50 | The Hylo vault accounts, then the NAV | "Collateral, times the SOL price, minus the debt, over supply. Four numbers from Hylo's books. Our program reads them on-chain, does that arithmetic on-chain, and writes the answer to an account. No server, no API key, no trust in us." |
| 0:50-1:30 | `check-pyth.py` output, red | "Here is why this matters. These are the price feeds people assume are live. Six days stale. Thirty-four days. Seventy-eight on devnet. Nobody noticed because nothing refuses — an oracle serves what it has and lets you decide." |
| 1:30-2:10 | The adapter refusing: `PythTooOld` | "Ours refuses. Wrong owner, wrong feed, too old, confidence too wide — six checks, and any one of them stops the write. The quote goes stale visibly instead of going wrong quietly. Fail stale, never fail wrong." |
| 2:10-2:40 | The stock pair: fair value vs market | "The same interface, a different asset. A tokenized Tesla share against the real one. Pyth publishes both; nobody publishes the gap. That gap is what a lender needs and what nobody is watching." |
| 2:40-3:00 | The dashboard, then the repo | "One account format, any asset, written only by code that can prove its inputs. Open source, permissionless to crank, no token. Read the accounts yourself — the addresses are on the page." |

### Three things never to say

- **"Fresher than Pyth."** Pyth publishes the input; we publish the output. A
  quote can never be fresher than the price it came from.
- **"Live on mainnet"** — until it is. Say where it runs, plainly, every time.
  A mainnet *fork* is a fork; call it one.
- **"Audited."** It is not, and nothing should be collateralised against these
  feeds until it is.

---

## 4. Technical video — 5 minutes

The audience is deciding whether this is real. They will forgive an unfinished
thing; they will not forgive a hidden one. Everything on screen is live: the
tests, the keeper, the refusals. No slides inside the demo.

| Time | Show | Point |
|---|---|---|
| 0:00-0:40 | `cargo test --test surfnet -- --nocapture` | It deploys both programs, re-snapshots Hylo's accounts from mainnet, pins the clock, refreshes, reads back a real NAV. Say that no Hylo or Pyth byte is altered — re-cloning is what a fresh fork does. |
| 0:40-1:30 | `svi-core`'s descriptor and the 320-byte quote | One account format for every asset. A write is accepted only from the adapter PDA named in the descriptor, so a compromised keeper cannot write a value. Show the layout test. |
| 1:30-2:30 | `pyth.rs`, then a deliberate `PythFeedMismatch` | Swap two price accounts in `deployments.json` and let it fail on camera. A real, fully verified Pyth price refused for being the wrong asset. This is the single best thirty seconds in the video. |
| 2:30-3:20 | `stock-check` agreeing with the adapter | An independent implementation sharing no code reaches the same number. Then show both quotes carrying the same `observed_slot` — one instruction, so the pair cannot be half-updated. |
| 3:20-4:10 | `check-pyth.py` against mainnet and devnet | The staleness finding, reproduced live. Then the three verdicts it gives and why they need different responses. |
| 4:10-5:00 | The limits, said plainly | No consumer reads the feed yet. No audit. No price source for the stock feeds — Hermes is gated and the sponsored accounts stopped. The xSOL feed needs none of that. Say what you would need to fix each. |

### Why the refusal is the demo

Anyone can publish a number. The work is in declining to. Spend the time there:
a judge who has seen forty oracle demos has never seen one refuse on camera.

---

## Before you submit

- [ ] Both videos uploaded and unlisted-or-public — not "private", which judges cannot open.
- [ ] Demo URL loads on a phone, and shows either a live quote or an honest empty state.
- [ ] `README.md` reaches a running thing in five minutes.
- [ ] Every number in the deck matches the repository today.
- [ ] Nothing claims mainnet, audit, or a consumer that does not exist.
- [ ] The repository is public, and a stranger can clone and test it.

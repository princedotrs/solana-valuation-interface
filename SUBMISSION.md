# Stocklana submission — SVI

Everything a submission form asks for, plus a checklist for the things only a
human at a keyboard can finish.

---

## Name

**SVI — the Solana Valuation Interface**

## Short description (under 180 characters)

> Tokenized stocks trade 24/7 but the real shares don't. SVI publishes both
> prices on-chain as verifiable accounts, flagged when the market is shut and
> when the token has drifted.

*(177 characters.)*

## Full description

> A tokenized stock like AAPLx trades on Solana twenty-four hours a day. The
> share it represents trades six and a half. For roughly two thirds of every
> week, the token has a price and the company does not — thin overnight books,
> no arbitrage possible, because the thing it tracks is closed.
>
> Nothing on-chain says so. A lending market reading the token's 3am price
> reads a number with no indication that it is a guess made in an empty room,
> and liquidates against it anyway. By the opening bell the price is back; the
> liquidation is not.
>
> **SVI puts the true value on-chain.** For each stock, one Solana transaction
> reads two Pyth feeds — the real equity (`Equity.US.AAPL/USD`) and the token
> (`Crypto.AAPLX/USD`) — and writes two quote accounts anyone can read:
>
> - `stock-<sym>-fair-value-v1` — what the real share is worth
> - `stock-<sym>-market-v1` — what the token trades at on Solana
>
> Both are written in the **same instruction**, so the pair can never be
> half-updated: there is no moment at which a fresh market price sits against
> a stale fair value, because there is no moment at which one exists without
> the other. Each carries Pyth's confidence interval as explicit `lower` and
> `upper` bounds, the slot it was observed at, and the slot past which a
> consumer must refuse it.
>
> **The flags are the product.** The value is the easy part. What makes this
> worth deploying is that the account says what kind of moment it was taken
> in:
>
> - `MARKET_CLOSED` — the equity price is over 15 minutes old. During a
>   session that cannot happen, so it means the exchange is shut. Not an
>   error: an equity feed is *supposed* to be hours old overnight. It is
>   information a consumer can act on.
> - `REFERENCE_STALE` — over four days, which no ordinary weekend or holiday
>   explains.
> - `TOKEN_FEED_STALE` — the token feed has gone quiet, which it never should.
> - `DEVIATION_HIGH` — the token is more than 2% from the real share.
>
> Past eight days, or with a confidence interval wider than 5%, the adapter
> publishes **nothing at all** and the previous quote expires. *Fail stale,
> never fail wrong* — a visibly expired quote stops a consumer; a plausible
> wrong one does not.
>
> The two-window design is the decision the whole product turns on. "Too old
> to use" and "old enough to mention" are different questions, and collapsing
> them makes the product impossible: the feed would go dark at exactly the
> moment you most need to be told the market is shut. All three thresholds
> live in the symbol's on-chain config, so a third party can read the rules
> that produced a quote rather than trusting a description of them.
>
> **Why you can believe it.** There is no API. The math runs in a Solana
> program, the inputs are Pyth accounts, the output is an account you read
> directly — four lines and about 200 compute units, no CPI. Cranking is
> permissionless and the keeper holds no privileged key: `svi-core` accepts
> the write only because the adapter program's own PDA signed the CPI. The
> 32-byte Pyth feed id inside each price account is re-checked on-chain on
> every refresh, so nobody can pass a real, fully verified price for a
> different, cheaper asset and have it published as Apple's.
>
> And `tools/stock-check` re-runs the whole thing using none of SVI's code.
> Its arithmetic is written out again from Pyth's documented convention, and
> its tests pin sixteen vectors against the code the adapter actually runs.
> Two implementations agreeing is evidence; one agreeing with itself is not.
>
> **It already generalises.** Before stocks, the same core published the NAV
> of Hylo's xSOL — a value computed by *subtraction* from a protocol's vault,
> with no market price anywhere and nothing resembling a Pyth feed. Same core,
> same 320-byte account, same write path, same refusal rules; only the adapter
> differs. On 10 September 2026 it published a real quote on a fork of Solana
> mainnet with Hylo's accounts re-cloned seconds before, and an independent
> off-chain reader agreed to the last digit: `$0.060285498`.
>
> That is the argument for calling this an *interface* rather than an oracle.
> An oracle gives you a number. An interface gives every kind of value a
> common way to be published, checked and refused.
>
> **Status, stated plainly.** Devnet, not mainnet. Unaudited. The adapter, the
> core, the keeper, the independent verifier, the deploy runbook and the live
> dashboard are written and tested. No lending market consumes it yet — that
> is the next thing, and it is the thing that would make it real.

## Pyth track

Pyth is the only price source, and it is used as more than a number:

- **Two feeds per symbol, read in one transaction**, so the drift between them
  is measured at a single instant rather than across two reads.
- **Confidence intervals become the published bounds.** Pyth's `conf` is
  carried into `lower`/`upper` with asymmetric rounding — lower floors, upper
  ceils — so the published band is never tighter than Pyth's. A quote whose
  confidence exceeds 500 bps is refused: an oracle that unsure is not
  publishing a price.
- **Verification level is enforced and recorded.** The adapter checks
  `VerificationLevel` against a per-symbol on-chain minimum, defaulting to
  `Full`. Where a deployment must post its own updates, the weaker level is
  written into the config where consumers can see it rather than hidden in a
  keeper's configuration.
- **Feed identity is checked on-chain every refresh** against the config's
  32-byte id — the check that stops a malicious keeper substituting a cheaper
  asset's price.
- **Feed ids are fetched from Hermes at build time**
  (`scripts/fetch-feed-ids.py`), never typed from memory, and the generated
  file records the URL and timestamp they came from.
- **The full Hermes → `post_update_atomic` → refresh path** is implemented,
  including trimming the Wormhole VAA's signature list to fit Solana's packet
  limit — which is precisely why that path yields Partial verification, and
  precisely why the level is a config field.

---

## Links

| | |
|---|---|
| Repository | https://github.com/princedotrs/solana-valuation-interface |
| Dashboard | *(fill in after the Netlify deploy)* |
| Pitch video (3 min) | *(fill in)* |
| Technical video (5 min) | *(fill in)* |
| Live addresses | `deployments.json` in the repo root |
| Validation record | `docs/validation/` |

---

## What is not done, and why

Stated here rather than discovered by a judge.

| | |
|---|---|
| **Devnet deployment** | The adapter, keeper, runbook and address planner are written and tested, but nothing is deployed. The deploy needs outbound access to `hermes.pyth.network` and `api.devnet.solana.com` plus the Anchor and Solana toolchains — none of which were reachable from the environment this was built in. It is `programs/svi-stock-adapter/runbooks/README.md`, four commands, on a machine with a wallet. |
| **A `MARKET_CLOSED` observation** | The flag is implemented and unit-tested, but observing it on-chain requires cranking across a real US market close. Until that run happens, the flag is described rather than demonstrated, and the README says so. |
| **`svi-sample-lender`** | Not built. The interface has no consumer yet, which is the honest gap — a feed nobody reads is a feed nobody has stress-tested. |
| **`svi-observer`** | Not built. `tools/stock-check` does the same job on demand; the observer would do it continuously. |
| **`svi-dbc-adapter`, PreStocks adapter** | Not built. They were scoped as stretch work behind a working devnet deployment, and the deployment comes first. |
| **`methodology_hash`** | Zero on every feed, because the spec document for the stock feeds is not frozen. A real deployment must set it; consumers compare it to detect a feed's definition changing under them. |
| **Audit** | None. Said on the site, in the README, and here. |

---

## Pre-submit checklist

Ordered so that each item can actually be done when you reach it.

### Deploy

- [ ] `python3 scripts/fetch-feed-ids.py AAPL TSLA NVDA` — check the output
      names real feeds and that `config/pyth-feeds.json` records the source URL
- [ ] `anchor keys sync && anchor build` in **both** `programs/svi-core` and
      `programs/svi-stock-adapter`
- [ ] `python3 scripts/plan-deployment.py --svi-core <ID> --adapter <ID>`
- [ ] `surfpool run publish --env devnet --unsupervised`
- [ ] `cargo run -p svi-keeper -- watch --feed stock:AAPL --once` returns a quote
- [ ] `cargo run --manifest-path tools/stock-check/Cargo.toml -- AAPL` says OK

### Observe the thing the product exists for

- [ ] Leave `crank --feed stock:AAPL --interval 30` running across a US market
      close (16:00 ET)
- [ ] Confirm `MARKET_CLOSED` appears on-chain, not just in the keeper's output
- [ ] Capture the run in `docs/validation/<date>-stocks-devnet.md`, with
      transaction signatures, the same way the Hylo record is written

### Publish

- [ ] Netlify: point it at the **repository root** (not `site/`), so
      `netlify.toml` copies `deployments.json` into the publish directory
- [ ] Load the dashboard and confirm it shows live values, not the
      "nothing published yet" state
- [ ] Fill the dashboard URL into this file and into README.md

### Check every claim

- [ ] `deployments.json` addresses match what the README and the dashboard show
- [ ] Every number in `docs/validation/` traces to a transaction signature
- [ ] The full suite is green: `cargo test` in each crate,
      `node --test site/test/*.mjs`, and the three `--self-test` scripts
- [ ] Search the repo for "mainnet" and confirm every use is either the Hylo
      **fork** run or a roadmap item
- [ ] Search for "audit" and confirm nothing claims one

### Repository hygiene

- [ ] **Rotate the Helius API key.** A real key is committed at
      `tools/nav-check/src/main.rs` as a test fixture and is in git history.
      Rotation is the only fix that counts — removing it from the file does
      not un-publish it.
- [x] `docs/context/` is **purged from history**, not just deleted.
      `git filter-repo --path docs/context --invert-paths` was run across every
      ref and all three branches were force-pushed, so no object under that
      path is reachable from anywhere in the repository. Verified by cloning
      the remote fresh and finding zero matching objects across all refs. Every
      commit SHA changed as a result: any existing clone of this repo is stale
      and needs re-cloning rather than pulling.
- [ ] Make the repository public
- [ ] Confirm the README renders correctly on GitHub — tables and all

### Record

- [ ] Follow `docs/product/09-pitch-video.md`: recording checklist first, then
      the two scripts
- [ ] Record during or just after a market close if at all possible
- [ ] Upload both cuts, fill the links into this file

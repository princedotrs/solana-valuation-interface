# Deploying the stock adapter to devnet

Four commands, in order. Each one's output feeds the next, so a step that
prints something unexpected is a step to stop on rather than push through.

Nothing here can be run from a sandbox without outbound access to
`hermes.pyth.network` and `api.devnet.solana.com`. It is written to be run on
your own machine.

---

## 0. Prerequisites

```
solana --version          # any 2.x
anchor --version          # 0.32.1 for this workspace
surfpool --version        # runs the txtx runbook
solana config set --url devnet
solana airdrop 5          # deploying two programs is not free
```

## 1. Get the feed ids from Pyth, not from memory

```
python3 scripts/fetch-feed-ids.py AAPL TSLA NVDA
```

Writes `config/pyth-feeds.json`, carrying the URL and timestamp each id came
from. A symbol missing either leg is skipped and named — pick different
tickers rather than half-configuring one, because a pair is the unit this
adapter publishes.

A feed id is 32 bytes that decide which price a program accepts. A wrong one
does not fail loudly; it publishes a real, fully-verified price for the wrong
asset. That is why this is a fetch and not a table of constants.

## 2. Build both programs, and read back their ids

```
cd programs/svi-core          && anchor keys sync && anchor build && anchor keys list
cd ../svi-stock-adapter       && anchor keys sync && anchor build && anchor keys list
```

`anchor keys sync` rewrites `declare_id!` to match the keypair in
`target/deploy/`. Skipping it deploys a program whose own id constant is
wrong, and every PDA the program derives is then derived under the wrong
program — which fails as a seeds-constraint error that names no cause.

## 3. Derive every address, once

```
python3 scripts/plan-deployment.py \
    --svi-core <SVI_CORE_ID> --adapter <STOCK_ADAPTER_ID>
```

Writes two files from one derivation:

- `deployments.json` — read by the keeper, the dashboard, and stock-check.
- `runbooks/publish/main.tx` — the txtx runbook that creates the accounts.

Neither is hand-edited. If they disagreed about an address the deployment
would not fail: it would create accounts at one set of addresses and read a
different set, and every reader would report "account not found" while the
runbook reported success.

The script refuses to write anything if two symbols derive a colliding
address, or if either program id is malformed.

## 4. Deploy and publish

```
cd programs/svi-stock-adapter
surfpool run publish --env devnet --unsupervised
```

`--env` is required. txtx skips any `signers.<env>.tx` whose middle component
does not match, and without it every action fails with
`unable to resolve 'signer.payer'`.

Do **not** fall back to `solana program deploy` if this fails. Against an
RPC-only endpoint it wants TPU and gossip it cannot reach, and a failed
redeploy leaves the previous binary running — so a deploy that did nothing
looks exactly like a code change that did nothing.

---

## Cranking

```
cargo run --manifest-path tools/svi-keeper/Cargo.toml -- \
    crank --feed stock:AAPL --interval 30

cargo run --manifest-path tools/svi-keeper/Cargo.toml -- \
    watch --feed stock:AAPL
```

Cranking is permissionless and signed by the payer. The keeper cannot
influence the published value: it chooses which accounts go into the
transaction, and the adapter re-checks the feed id inside each price account
before using it.

## What to expect at the US close

The point of the two-feed design is visible only across a session boundary,
so the validation run has to span one.

| when | equity feed age | flags on both quotes |
|---|---|---|
| during the session | seconds | none |
| ~15 min after 16:00 ET | > 900s | `MARKET_CLOSED` |
| overnight, token drifting | hours | `MARKET_CLOSED`, and `DEVIATION_HIGH` once the token is >2% from the last real print |
| after 4 days shut | > 4 days | adds `REFERENCE_STALE` |
| after 8 days | > 8 days | refusal, not a flag — `PythTooOld` (6007) |

Leave `crank` running through a close, keep the `watch` output, and put it in
`docs/validation/<date>-stocks-devnet.md`. A screenshot of `MARKET_CLOSED`
appearing on-chain by itself is the whole pitch.

## If the refresh fails

Every refusal has its own code, and the keeper prints what each one means.
The ones you will actually hit:

| code | name | what to do |
|---|---|---|
| 6000 | `PythWrongOwner` | the price account address is wrong — re-run step 3 |
| 6002 | `PythNotFullyVerified` | the feed is not sponsored on devnet; see below |
| 6003 | `PythFeedMismatch` | right account shape, wrong asset — re-run step 1 |
| 6007 | `PythTooOld` | outside `reference_max_age_secs`; genuinely broken, not closed |
| 6008 | `PythConfidenceTooWide` | Pyth has stopped having an opinion on this asset |

`PythNotFullyVerified` is the one with a real decision behind it. The adapter
defaults to requiring Pyth's **Full** verification, which exists only for
feeds Pyth itself keeps updated. Posting your own update in a single
transaction (`post_update_atomic`) produces a **Partial** update, which fewer
colluding guardians could forge. If a symbol has no sponsored feed on your
cluster, the choice is to post updates yourself and set that symbol's
`min_verification_level` to 0 — recorded on-chain, so a consumer can see
which feeds run at which level rather than having to trust a keeper's config.

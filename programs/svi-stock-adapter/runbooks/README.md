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

Then ask the cluster whether those feeds are actually usable on it:

```
python3 scripts/check-pyth.py --rpc https://api.devnet.solana.com
```

Read its verdict before going further -- it decides a config value that cannot
be changed after a symbol is created. See "Do it locally first" below.

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

### If `fetch-feed-ids.py` cannot reach Hermes

Two failures look like an outage and are not:

**`CERTIFICATE_VERIFY_FAILED: unable to get local issuer certificate`** — this
machine cannot verify TLS. A python.org build on macOS ships its own empty
trust store and never reads the system keychain, which is usually the cause:

```
python3 -m pip install certifi
export SSL_CERT_FILE="$(python3 -m certifi)"
```

Add that `export` to `~/.zshrc` to make it stick. It needs no installer and no
`sudo`, and it is the only step most machines need.

macOS also ships a fixer at `/Applications/Python 3.x/Install Certificates.command`,
but plenty of installs do not have that folder at all -- check with
`ls /Applications | grep -i python` before hunting for it. When it is missing:

```
python3 scripts/netutil.py --link-certifi
```

which links the certifi bundle into the exact path OpenSSL checks, so every
tool using that Python is fixed rather than just these scripts. It refuses to
touch a trust store that is a real file rather than a symlink, and says to use
the environment variable instead if the directory is root-owned.

**`Tunnel connection failed: 403 Forbidden`** — an HTTPS proxy refused the
host. That is a network policy, not a bug, and no other ticker will work.

Either way the deployment does not have to stop. Fetch the feeds anywhere with
access and pass the response in:

```
curl -s 'https://hermes.pyth.network/v2/price_feeds?query=AAPL' > aapl.json
python3 scripts/fetch-feed-ids.py --from-file aapl.json AAPL
```

The file must contain both legs — `Equity.US.<SYM>/USD` and `Crypto.<SYM>X/USD`.
Filtering and pairing are identical either way, so a file and a live fetch
produce the same config.

---

## Do it locally first

Everything below runs against a Surfpool fork of mainnet, so the adapter reads
the same Pyth bytes mainnet has, with no devnet SOL spent and nothing published
that anyone will see. Get a green run here before step 4 above.

A bare `solana-test-validator` will not work: the Pyth receiver program, its
config and treasury accounts, the Wormhole guardian sets and the sponsored
price accounts all have to exist, and a fork is the cheap way to have them.

```
surfpool start                                  # terminal 1, forks mainnet
```

```
# terminal 2
python3 scripts/fetch-feed-ids.py AAPL TSLA NVDA
python3 scripts/check-pyth.py --rpc http://127.0.0.1:8899

cd programs/svi-core          && anchor keys sync && anchor build && anchor keys list
cd ../svi-stock-adapter       && anchor keys sync && anchor build && anchor keys list

python3 scripts/plan-deployment.py \
    --svi-core <ID> --adapter <ID> --cluster localnet

cd programs/svi-stock-adapter
surfpool run publish --env localnet --unsupervised
```

### Read `check-pyth.py` before you go further

It is the step that decides how the rest of the deployment is configured, and
the decision cannot be changed afterwards:

- **"READ DIRECTLY"** — Pyth sponsors both feeds on this cluster. Leave
  `min_verification_level = 1` and crank normally.
- **"NEEDS `--post-updates`"** — no sponsored account, so the keeper brings the
  price from Hermes. That posts atomically, which checks only a subset of
  guardian signatures, so the update is **Partial** and the symbol must be
  created with `min_verification_level = 0`.

`min_verification_level` is set by `initialize_symbol` and **there is no
instruction to change it**. The config PDA is seeded by the ticker, so a symbol
created with the wrong value cannot be re-initialised — the account already
exists. Recovering means deploying the adapter under a new program id.

Get this right on the localnet run, where it costs nothing.

### Then verify, in this order

```
# terminal 3
cargo run --manifest-path tools/svi-keeper/Cargo.toml -- \
    crank --feed stock:AAPL --interval 15 --rpc http://127.0.0.1:8899

# terminal 4
cargo run --manifest-path tools/svi-keeper/Cargo.toml -- \
    watch --feed stock:AAPL --rpc http://127.0.0.1:8899

# terminal 5 — the independent check
cargo run --manifest-path tools/stock-check/Cargo.toml -- \
    AAPL --rpc http://127.0.0.1:8899
```

What a green run looks like, and what each thing proves:

| check | proves |
|---|---|
| the crank prints a fair value and a market price | both feeds verified and both quotes written |
| `stock-check` prints `OK` | an implementation sharing no code agrees |
| both quotes report the **same** `observed_slot` | they came from one instruction, so the pair cannot be half-updated |
| `value_type` 7 on fair, 2 on market | the feeds are typed as consumers expect |
| the dashboard shows values | the 320-byte layout decodes the same in the browser |

Then deliberately break it, because the refusals are the product:

```
# point the keeper at the wrong price account and watch it abort
# (edit deployments.json, swap equity_price and token_price)
```

Expect `PythFeedMismatch (6003)` — a real, fully verified Pyth price being
refused because it is the wrong asset. That is the check that stops a
malicious keeper, and seeing it fire is worth more than reading about it.

### Only then go to devnet

Re-run step 3 with `--cluster devnet`, re-run `check-pyth.py` against devnet
(sponsorship differs per cluster, so the answer can change), and deploy.

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

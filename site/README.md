# The public site

One HTML file, no build step, no dependencies beyond Google Fonts.

```bash
cp deployments.json site/deployments.json   # so the live panel has something to read
python3 -m http.server -d site 8080         # then open localhost:8080
node --test site/test/*.mjs                 # the decoder's tests
```

Deploying to Netlify: point it at the **repository root**, not at `site/`. The
root `netlify.toml` sets the publish directory and copies `deployments.json`
into it, which the live dashboard needs. Drag-and-drop of `site/` works for a
quick look but leaves the dashboard with nothing to read.

## The live section

The panel at the top reads devnet quote accounts over a public RPC, from the
browser, with no backend. It decodes the 320-byte payload by the offsets
`svi-core/tests/core.rs::layout_is_frozen` asserts.

A wrong offset would not throw — it would read a neighbouring field and render
a plausible number, which is the worst failure available to a page whose whole
claim is that you can check it yourself. So `site/test/decode.test.mjs` parses
the offset table out of the shipped page and compares it to the frozen layout,
then runs the decoder over a synthetic account whose every field differs.

When there is nothing to read, the panel says so. It never shows a
last-known-good value, for the same reason the on-chain code never publishes
one.

## The three interactive pieces

All three compute from the real state recorded in
`docs/validation/2026-09-10-xsol-nav-mainnet.md`. None of them invents data.

| | What it does | Why it is honest |
|---|---|---|
| **Where the number comes from** | SOL price slider → collateral, equity, NAV, leverage, collateral ratio, zone flags | Constants are Hylo's real books at slot 445953445. At the observed SOL price it lands exactly on the published quote. The page says which parts are simplified. |
| **Fail stale** | Slot slider → VALID/STALE and the lending decision | Arithmetic on the published `observed_slot` and `valid_until_slot`. |
| **Recorded run** | Replays the `cargo test --test surfnet` output | Verbatim from the run that produced the first publication, labelled as a recording. |

## Rules this page has to keep

- Every number traceable to `docs/validation/`, to a live account, or to the code.
- "Mainnet fork" said plainly wherever the Hylo run is mentioned — it is not mainnet.
- "Devnet" said plainly wherever the stock feeds are mentioned.
- Never "fresher than Pyth". Pyth publishes the input; SVI publishes the
  output; a quote can never be fresher than the price it came from.
- The unfinished parts stay listed as unfinished.

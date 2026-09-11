# svi.site

The public site: what SVI is, the proof it works, and three things you can
poke at. One file, no build step, no dependencies beyond Google Fonts.

```bash
python3 -m http.server -d site 8080    # then open localhost:8080
```

Deploying to Netlify: point it at this repo with publish directory `site/`
and an empty build command (`netlify.toml` already says so). Drag-and-drop of
the folder works too.

## The three interactive pieces

All three compute from the real state recorded in
`docs/validation/2026-09-10-xsol-nav-mainnet.md`. None of them invents data.

| | What it does | Why it is honest |
|---|---|---|
| **Where the number comes from** | SOL price slider → collateral, equity, NAV, leverage, collateral ratio, zone flags | Constants are Hylo's real books at slot 445953445. At the observed SOL price it lands exactly on the published quote. The page says which parts are simplified. |
| **Fail stale** | Slot slider → VALID/STALE and the lending decision | Arithmetic on the published `observed_slot` and `valid_until_slot`. |
| **Recorded run** | Replays the `cargo test --test surfnet` output | Verbatim from the run that produced the first publication, labelled as a recording. |

## Rules this page has to keep

- Every number traceable to `docs/validation/` or the code.
- "Mainnet fork" said plainly wherever the run is mentioned — it is not mainnet.
- Never "fresher than Pyth". Pyth publishes the input; SVI publishes the
  output; the quote can never be fresher than the price it came from.
- The unfinished parts stay listed as unfinished.

Update the figures here whenever a new validation record lands.

# Presentation materials

| File | What it is |
|---|---|
| `svi-pitch-deck.pptx` | **Current.** 19-slide public pitch — Twitter, Superteam, Colosseum, lenders. Leads with the on-chain receipt. |
| `svi-pitch-deck.pdf` | Same deck, flattened — send this one. |
| `build_deck.js` | The generator. The deck is built, not hand-edited. |
| `svi-hylo-partnership-deck.pptx` / `.pdf` | **Superseded.** The earlier Hylo design-partner proposal. Kept because its five questions (§10 of the spec) still stand if Hylo ever engages. |

Speaker notes are on slides 1–5, 7, 8, 11, 14, 17.

## Structure

| Slides | Section | Purpose |
|---|---|---|
| 1–2 | **Proof first** | The first on-chain publication, with the numbers |
| 3–6 | **Problem** | Plish's public post, the five trusted hops, four incidents, the idea |
| 7–12 | **How it works** | Architecture, where Pyth sits, five pieces, one refresh, fail-stale, three ways to consume |
| 13 | **Status** | What is done, proven, drafted, and next |
| 14–16 | **Why it exists** | Market, landscape, roadmap |
| 17–19 | **The ask** | Three audiences, objections answered, what happens next |

## Regenerating

```bash
python3 docs/diagrams/gen_architecture.py     # slide 7
python3 docs/diagrams/gen_trustchain.py       # slide 4
NODE_PATH=/path/to/node_modules node docs/deck/build_deck.js   # needs pptxgenjs
```

Every claim is a compression of `docs/product/` and `docs/validation/`; those
are the source of truth. The video plan and script are in
`docs/product/09-pitch-video.md`.

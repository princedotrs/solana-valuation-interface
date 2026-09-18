# Presentation materials

| File | What it is |
|---|---|
| `svi-pitch-deck.pptx` | **Current.** 20-slide public pitch for the Stocklana submission. Leads with the tokenized-stock problem; the Hylo publication becomes the proof that the interface generalises. |
| `svi-pitch-deck.pdf` | Same deck, flattened — send this one. |
| `build_deck.js` | The generator. The deck is built, not hand-edited. |
| `svi-hylo-partnership-deck.pptx` / `.pdf` | **Superseded.** The earlier Hylo design-partner proposal. Kept because its five questions (§10 of the spec) still stand if Hylo ever engages. |

Speaker notes are on slides 1–5, 7, 8, 11, 14, 17.

## Structure

| Slides | Section | Purpose |
|---|---|---|
| 1 | **The hook** | AAPLx trades all night. Apple doesn't. |
| 2 | **Adapter #2** | The Hylo publication — proof the interface generalises |
| 3–4 | **Problem and fix** | Two thirds of every week; the two feeds and the flags |
| 5–7 | **Why it matters** | The trust chain, four incidents, the idea |
| 8–13 | **How it works** | Architecture, where Pyth sits, five pieces, one refresh, fail-stale, three ways to consume |
| 14 | **Status** | What is done and what is next, including what is not |
| 15–17 | **Why it exists** | Market, landscape, roadmap |
| 18–20 | **The ask** | Three audiences, objections answered, what happens next |

## Regenerating

```bash
python3 docs/diagrams/gen_architecture.py     # slide 7
python3 docs/diagrams/gen_trustchain.py       # slide 4
NODE_PATH=/path/to/node_modules node docs/deck/build_deck.js   # needs pptxgenjs

soffice --headless --convert-to pdf docs/deck/svi-pitch-deck.pptx \
        --outdir docs/deck                                       # the PDF
```

**Check the slides you changed.** LibreOffice will silently let a table run
off the bottom of a slide, and the PPTX gives no warning. Render and look:

```bash
python3 -c "import pymupdf; d=pymupdf.open('docs/deck/svi-pitch-deck.pdf'); \
            d[13].get_pixmap(dpi=72).save('/tmp/s.png')"
```

Every claim is a compression of `docs/product/` and `docs/validation/`; those
are the source of truth. The video plan and script are in
`docs/product/09-pitch-video.md`.

# Presentation materials

| File | What it is |
|---|---|
| `svi-hylo-partnership-deck.pptx` | 21-slide partnership deck for the Hylo meeting. Editable in PowerPoint, Keynote or Google Slides. |
| `svi-hylo-partnership-deck.pdf` | Same deck, flattened — use this when sending it to someone. |

Speaker notes are attached to the slides that need them (1, 2, 3, 4, 5, 6, 9, 12, 15, 18).

## Structure

| Slides | Section | Purpose |
|---|---|---|
| 1–4 | **Problem** | Plish's own words, the five trusted hops, and four incidents where this exact failure class cost real money |
| 5–10 | **Solution** | The idea, the architecture, the five pieces, the refresh flow, the failure matrix, the three consumption modes |
| 11–14 | **Why Hylo says yes** | Zero program changes, we reimplement none of your math, what's already built, what Hylo gets |
| 15–17 | **Why this exists at all** | Market, competitive landscape, go-to-market — including the parts that argue against it |
| 18–21 | **The ask** | Five questions, the pilot plan, honest objections, next steps |

## Regenerating

The deck is generated, not hand-edited. The two full-bleed diagram slides (3 and 6)
embed PNGs from [`../diagrams/`](../diagrams/) — regenerate those first if the
architecture changes:

```bash
python3 ../diagrams/gen_architecture.py
python3 ../diagrams/gen_trustchain.py
```

The generator script itself is not checked in (it depends on a local `pptxgenjs`
install). Edit the `.pptx` directly for small changes; for structural changes,
rebuild from the source docs in [`../product/`](../product/) — the deck is a
compression of those, and they are the source of truth for every claim in it.

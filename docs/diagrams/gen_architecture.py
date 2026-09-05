#!/usr/bin/env python3
"""Generate the SVI architecture diagram as SVG (and a PNG for the deck).

Hand-laid-out rather than mermaid-rendered so it stays legible at slide size.
Run:  python3 gen_architecture.py
"""
from pathlib import Path

W, H = 1600, 1010
FONT = "Arial, Helvetica, 'Liberation Sans', sans-serif"
MONO = "'DejaVu Sans Mono', Menlo, Consolas, monospace"

PALETTE = {
    "ext":  ("#eef1f5", "#94a3b8"),
    "svi":  ("#ede9fe", "#7c3aed"),
    "prod": ("#fef3c7", "#d97706"),
    "cons": ("#dbeafe", "#2563eb"),
    "ops":  ("#dcfce7", "#16a34a"),
}
INK = "#0f172a"
MUTED = "#475569"
ARROW = "#64748b"

out = []
def add(s): out.append(s)


def esc(t):
    return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def box(x, y, w, h, kind, title, lines=(), r=10, title_size=19, line_size=14,
        mono_lines=()):
    fill, stroke = PALETTE[kind]
    add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" '
        f'fill="{fill}" stroke="{stroke}" stroke-width="2"/>')
    cx = x + w / 2
    n = len(lines) + len(mono_lines)
    # vertically centre the title+lines block
    block_h = title_size + 6 + n * (line_size + 5)
    ty = y + (h - block_h) / 2 + title_size
    add(f'<text x="{cx}" y="{ty}" font-family="{FONT}" font-size="{title_size}" '
        f'font-weight="700" fill="{INK}" text-anchor="middle">{esc(title)}</text>')
    ly = ty + 8
    for ln in lines:
        ly += line_size + 5
        add(f'<text x="{cx}" y="{ly}" font-family="{FONT}" font-size="{line_size}" '
            f'fill="{MUTED}" text-anchor="middle">{esc(ln)}</text>')
    for ln in mono_lines:
        ly += line_size + 5
        add(f'<text x="{cx}" y="{ly}" font-family="{MONO}" font-size="{line_size - 1}" '
            f'fill="{MUTED}" text-anchor="middle">{esc(ln)}</text>')


def group(x, y, w, h, label, stroke="#cbd5e1", dash="6 5"):
    add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="14" fill="none" '
        f'stroke="{stroke}" stroke-width="1.5" stroke-dasharray="{dash}"/>')
    add(f'<text x="{x + 18}" y="{y + 25}" font-family="{FONT}" font-size="14" '
        f'font-weight="700" fill="#64748b" letter-spacing="1.1">{esc(label)}</text>')


def arrow(x1, y1, x2, y2, dashed=False, color=ARROW, width=2.2, marker="head"):
    d = ' stroke-dasharray="7 5"' if dashed else ""
    add(f'<path d="M {x1} {y1} L {x2} {y2}" stroke="{color}" stroke-width="{width}" '
        f'fill="none" marker-end="url(#{marker})"{d}/>')


def elbow(x1, y1, x2, y2, dashed=False, color=ARROW, width=2.2):
    """Vertical-then-horizontal elbow."""
    d = ' stroke-dasharray="7 5"' if dashed else ""
    add(f'<path d="M {x1} {y1} V {y2} H {x2}" stroke="{color}" stroke-width="{width}" '
        f'fill="none" marker-end="url(#head)"{d}/>')


def label(x, y, text, size=13, color=MUTED, anchor="middle", weight="600",
          bg=True, font=FONT):
    if bg:
        wpx = len(text) * size * 0.53 + 16
        lx = {"middle": x - wpx / 2, "start": x - 8, "end": x - wpx + 8}[anchor]
        add(f'<rect x="{lx}" y="{y - size + 2}" width="{wpx}" height="{size + 8}" '
            f'rx="4" fill="#ffffff" opacity="0.92"/>')
    add(f'<text x="{x}" y="{y}" font-family="{font}" font-size="{size}" '
        f'font-weight="{weight}" fill="{color}" text-anchor="{anchor}">{esc(text)}</text>')


# ---------------------------------------------------------------- canvas
add(f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
    f'viewBox="0 0 {W} {H}" font-family="{FONT}">')
add('<defs>')
add(f'<marker id="head" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" '
    f'markerHeight="7" orient="auto-start-reverse">'
    f'<path d="M 0 0 L 10 5 L 0 10 z" fill="{ARROW}"/></marker>')
add(f'<marker id="headred" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" '
    f'markerHeight="7" orient="auto-start-reverse">'
    f'<path d="M 0 0 L 10 5 L 0 10 z" fill="#dc2626"/></marker>')
add('</defs>')
add(f'<rect width="{W}" height="{H}" fill="#ffffff"/>')

add(f'<text x="60" y="52" font-family="{FONT}" font-size="30" font-weight="700" '
    f'fill="{INK}">Solana Valuation Interface — system architecture</text>')
add(f'<text x="60" y="82" font-family="{FONT}" font-size="17" fill="{MUTED}">'
    f'The value is computed on-chain from verified accounts and written to a public account. '
    f'No off-chain service is ever trusted with the arithmetic.</text>')

# ---------------------------------------------------------------- sources
group(60, 108, 1180, 152, "EXISTING ON-CHAIN STATE  ·  UNMODIFIED  ·  NO COOPERATION REQUIRED")
src = [
    (90,  "Hylo Exchange", ["protocol state · LST headers", "TotalSolCache · xSOL mint"]),
    (375, "Sanctum / SPL", ["stake pools", "LST redemption rates"]),
    (660, "Pyth SOL/USD", ["PriceUpdateV2", "price + confidence"]),
    (945, "Clock sysvar", ["slot · epoch", "unix timestamp"]),
]
for x, t, ls in src:
    box(x, 142, 265, 100, "ext", t, ls, title_size=17, line_size=13)

# ---------------------------------------------------------------- adapter
box(340, 316, 620, 140, "svi", "svi-hylo-adapter   —   the calculator",
    ["verify owner · PDA derivation · mint · program ID",
     "call hylo-core (pinned SHA) via svi-math — exact integers, no floats",
     "propagate Pyth confidence into lower / upper bounds"],
    title_size=21, line_size=14)

box(60, 342, 250, 92, "ops", "Keeper",
    ["permissionless crank", "pays gas · cannot", "influence the value"],
    title_size=17, line_size=12)

# ---------------------------------------------------------------- core
box(340, 500, 620, 108, "svi", "svi-core   —   the notice board",
    ["dumb, auditable, freezable · knows nothing about Hylo",
     "adapter PDA signed? · sequence++ · fresh? · lower ≤ v ≤ upper?"],
    title_size=21, line_size=14)

# ---------------------------------------------------------------- quote
box(240, 658, 860, 128, "prod", "QUOTE ACCOUNT   —   320 bytes, fixed layout   —   the product",
    ["value + lower / upper bounds  ·  observed_slot  ·  valid_until_slot",
     "sequence  ·  status_flags  ·  methodology_hash  ·  value_type"],
    title_size=21, line_size=14.5)

# ---------------------------------------------------------------- observer
box(1155, 658, 280, 128, "ops", "Independent observer",
    ["recomputes NAV from raw", "accounts via 2 RPCs and", "compares, byte for byte"],
    title_size=18, line_size=13)

# ---------------------------------------------------------------- consumers
group(60, 830, 1480, 150, "CONSUMERS")
box(105,  862, 340, 96, "cons", "Lending markets",
    ["Kamino · marginfi · Loopscale", "use lower bound as collateral"], title_size=18, line_size=13)
box(475,  862, 280, 96, "cons", "Kamino Scope",
    ["as an SVI quote type"], title_size=18, line_size=13)
box(785,  862, 280, 96, "cons", "Wallets · dashboards",
    ["display NAV honestly"], title_size=18, line_size=13)
box(1095, 862, 400, 96, "cons", "Verifiable mirror API  →  Pyth",
    ["ZERO arithmetic · 2 RPCs · byte-for-byte match", "the API Pyth asked for, made safe"],
    title_size=18, line_size=13)

# ---------------------------------------------------------------- arrows
for x, _, _ in src:
    arrow(x + 132, 242, 650, 310)
label(650, 288, "accounts passed into the transaction — all reads same-slot", size=13)

arrow(310, 388, 334, 384)

arrow(650, 456, 650, 494)
label(650, 484, "CPI  ·  signed by the adapter PDA", size=13.5, color="#7c3aed")

arrow(650, 608, 650, 652)
label(650, 640, "the only writer", size=13.5, color="#d97706")

# quote -> consumers
arrow(670, 786, 275, 856)
arrow(670, 786, 615, 856)
arrow(670, 786, 925, 856)
arrow(670, 786, 1250, 856)

# observer: re-read the same source accounts, independently
add(f'<path d="M 1240 186 H 1295 V 652" stroke="{ARROW}" stroke-width="2" fill="none" '
    f'stroke-dasharray="7 5" marker-end="url(#head)"/>')
label(1300, 430, "independent re-read of", size=13, anchor="start")
label(1300, 450, "the same source accounts", size=13, anchor="start")

# observer -> quote: compare
arrow(1151, 722, 1106, 722, dashed=True)
label(1128, 700, "compare", size=12.5)

# observer -> mirror: halt on divergence
add(f'<path d="M 1295 786 V 856" stroke="#dc2626" stroke-width="2.4" fill="none" '
    f'stroke-dasharray="7 5" marker-end="url(#headred)"/>')
label(1295, 826, "halt on divergence", size=13, color="#dc2626")

# ---------------------------------------------------------------- footer rule
add(f'<text x="60" y="{H - 14}" font-family="{FONT}" font-size="14.5" fill="{MUTED}">'
    f'<tspan font-weight="700" fill="{INK}">Fail stale, never fail wrong.</tspan>'
    f'  Any unverified input aborts the refresh — the quote ages out and consumers see the expiry. '
    f'No code path publishes a value computed from unvalidated inputs.</text>')
add('</svg>')

svg = "\n".join(out)
here = Path(__file__).parent
(here / "svi-architecture.svg").write_text(svg)
print(f"wrote svi-architecture.svg ({len(svg)} bytes)")

try:
    import cairosvg
    cairosvg.svg2png(bytestring=svg.encode(), write_to=str(here / "svi-architecture.png"),
                     output_width=W * 2, output_height=H * 2, background_color="#ffffff")
    print("wrote svi-architecture.png @2x")
except ImportError:
    print("cairosvg not available — SVG only")

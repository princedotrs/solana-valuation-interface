#!/usr/bin/env python3
"""Before/after trust-chain diagram: the pipeline Pyth asked for vs. SVI."""
from pathlib import Path

W, H = 1600, 960
FONT = "Arial, Helvetica, 'Liberation Sans', sans-serif"
INK, MUTED, ARROW = "#0f172a", "#475569", "#94a3b8"

RED = ("#fee2e2", "#dc2626")
GREEN = ("#dcfce7", "#16a34a")
AMBER = ("#fef3c7", "#d97706")
NEUTRAL = ("#eef1f5", "#94a3b8")

out = []
add = out.append
def esc(t): return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

def node(x, y, w, h, colors, title, sub=None, tag=None):
    fill, stroke = colors
    add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="9" fill="{fill}" '
        f'stroke="{stroke}" stroke-width="2"/>')
    cx = x + w / 2
    if sub:
        add(f'<text x="{cx}" y="{y + h/2 - 3}" font-size="19" font-weight="700" fill="{INK}" '
            f'text-anchor="middle">{esc(title)}</text>')
        add(f'<text x="{cx}" y="{y + h/2 + 19}" font-size="14" fill="{MUTED}" '
            f'text-anchor="middle">{esc(sub)}</text>')
    else:
        add(f'<text x="{cx}" y="{y + h/2 + 7}" font-size="19" font-weight="700" fill="{INK}" '
            f'text-anchor="middle">{esc(title)}</text>')
    if tag:
        add(f'<rect x="{x + w - 104}" y="{y + 10}" width="92" height="22" rx="11" '
            f'fill="{stroke}"/>')
        add(f'<text x="{x + w - 58}" y="{y + 26}" font-size="12.5" font-weight="700" '
            f'fill="#ffffff" text-anchor="middle" letter-spacing="0.6">{esc(tag)}</text>')

def down(x, y1, y2, color=ARROW):
    add(f'<path d="M {x} {y1} L {x} {y2}" stroke="{color}" stroke-width="2.4" fill="none" '
        f'marker-end="url(#h)"/>')

add(f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" '
    f'font-family="{FONT}">')
add(f'<defs><marker id="h" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" '
    f'orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{ARROW}"/></marker></defs>')
add(f'<rect width="{W}" height="{H}" fill="#ffffff"/>')

add(f'<text x="60" y="54" font-size="31" font-weight="700" fill="{INK}">'
    f'Where does the number come from?</text>')
add(f'<text x="60" y="86" font-size="17" fill="{MUTED}">'
    f'Five places the value can change without anyone noticing — versus one, whose output anyone can '
    f'reproduce from the same slot\u2019s account data.</text>')

LX, RX, BW, BH, P = 90, 870, 620, 70, 92

# column headers
add(f'<rect x="{LX}" y="122" width="{BW}" height="46" rx="9" fill="#fef2f2" stroke="{RED[1]}" stroke-width="2"/>')
add(f'<text x="{LX + BW/2}" y="152" font-size="19" font-weight="700" fill="{RED[1]}" '
    f'text-anchor="middle">TODAY — the pipeline Pyth asked Hylo to build</text>')
add(f'<rect x="{RX}" y="122" width="{BW}" height="46" rx="9" fill="#f0fdf4" stroke="{GREEN[1]}" stroke-width="2"/>')
add(f'<text x="{RX + BW/2}" y="152" font-size="19" font-weight="700" fill="{GREEN[1]}" '
    f'text-anchor="middle">WITH SVI</text>')

left = [
    (NEUTRAL, "Hylo\u2019s on-chain accounts", "the only real source of truth", None),
    (RED, "One RPC provider", "stale or forked view goes undetected", "TRUSTED"),
    (RED, "Custom off-chain parser", "layout drift misreads silently", "TRUSTED"),
    (RED, "Private calculation code", "nobody downstream can reproduce it", "TRUSTED"),
    (RED, "Hylo-operated API", "server, DNS, dependency, insider", "TRUSTED"),
    (RED, "Publisher process + key", "a key can sign any number", "TRUSTED"),
    (NEUTRAL, "Price account on-chain", "consumed as collateral value", None),
]
right = [
    (NEUTRAL, "Hylo\u2019s on-chain accounts", "unmodified — no cooperation needed", None),
    (GREEN, "Adapter program", "public code, on-chain integer math", "VERIFIABLE"),
    (AMBER, "Quote account", "canonical, public, 320 fixed bytes", None),
    (GREEN, "Mirror API", "zero arithmetic — copies account bytes", "VERIFIABLE"),
    (RED, "Publisher process + key", "still Pyth\u2019s — SVI does not fix this", "TRUSTED"),
    (NEUTRAL, "Price account on-chain", "now reproducible from chain state", None),
]

y = 186
for i, (c, t, s, tag) in enumerate(left):
    node(LX, y, BW, BH, c, t, s, tag)
    if i < len(left) - 1:
        down(LX + BW/2, y + BH + 3, y + P - 5)
    y += P

y = 186
for i, (c, t, s, tag) in enumerate(right):
    node(RX, y, BW, BH, c, t, s, tag)
    if i < len(right) - 1:
        down(RX + BW/2, y + BH + 3, y + P - 5)
    y += P

# summary badges
add(f'<rect x="{LX}" y="830" width="{BW}" height="66" rx="10" fill="#fef2f2" stroke="{RED[1]}" stroke-width="2"/>')
add(f'<text x="{LX + BW/2}" y="859" font-size="24" font-weight="700" fill="{RED[1]}" '
    f'text-anchor="middle">5 trusted hops</text>')
add(f'<text x="{LX + BW/2}" y="882" font-size="14.5" fill="{MUTED}" text-anchor="middle">'
    f'and no way for a consumer to check any of them</text>')

add(f'<rect x="{RX}" y="830" width="{BW}" height="66" rx="10" fill="#f0fdf4" stroke="{GREEN[1]}" stroke-width="2"/>')
add(f'<text x="{RX + BW/2}" y="859" font-size="24" font-weight="700" fill="{GREEN[1]}" '
    f'text-anchor="middle">1 trusted hop — Pyth\u2019s own key</text>')
add(f'<text x="{RX + BW/2}" y="882" font-size="14.5" fill="{MUTED}" text-anchor="middle">'
    f'everything upstream is reproducible by anyone</text>')

add(f'<text x="60" y="{H - 14}" font-size="14.5" fill="{MUTED}">'
    f'<tspan font-weight="700" fill="{INK}">RPC is not the problem.</tspan>  '
    f'Every off-chain service reads the chain somehow. The question is where the arithmetic happens — '
    f'once it happens on-chain, RPCs, APIs and keys are couriers, not authors.</text>')
add('</svg>')

svg = "\n".join(out)
here = Path(__file__).parent
(here / "svi-trust-chain.svg").write_text(svg)
print("wrote svi-trust-chain.svg")
try:
    import cairosvg
    cairosvg.svg2png(bytestring=svg.encode(), write_to=str(here / "svi-trust-chain.png"),
                     output_width=W * 2, output_height=H * 2, background_color="#ffffff")
    print("wrote svi-trust-chain.png @2x")
except ImportError:
    print("cairosvg missing")

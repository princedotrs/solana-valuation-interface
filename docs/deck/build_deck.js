// Generates docs/deck/svi-pitch-deck.pptx — the public pitch (Twitter,
// Superteam, Colosseum). Run with pptxgenjs on NODE_PATH:
//
//   NODE_PATH=/path/to/node_modules node docs/deck/build_deck.js
//
// Every claim here is a compression of docs/product/ and docs/validation/;
// those are the source of truth. The on-chain figures are from the run on
// 18 Sep 2026 (docs/validation/2026-09-18-xsol-nav-surfnet.md); the 10 Sep
// record covers the earlier read-only recomputation.
const pptxgen = require("pptxgenjs");
const path = require("path");

const REPO = path.resolve(__dirname, "../..");
const DIA = path.join(REPO, "docs/diagrams");
const OUT = path.join(REPO, "docs/deck/svi-pitch-deck.pptx");

// ---------------------------------------------------------------- palette
const C = {
  ink:      "0F172A",
  midnight: "16132B",
  deep:     "241E45",
  violet:   "7C3AED",
  violetLt: "C4B5FD",
  paper:    "FFFFFF",
  tint:     "F6F5FB",
  line:     "E2E0EC",
  muted:    "5B6478",
  green:    "16A34A",
  greenBg:  "EAF7EF",
  red:      "DC2626",
  redBg:    "FDECEC",
  amber:    "D97706",
  amberBg:  "FEF3C7",
  blue:     "2563EB",
  blueBg:   "E5EEFC",
};
const F = "Arial";
const MONO = "Courier New";
const W = 13.333, H = 7.5, M = 0.62;
const CW = W - 2 * M;

const pres = new pptxgen();
pres.layout = "LAYOUT_WIDE";
pres.author = "Prince — SVI";
pres.title = "SVI — Solana Valuation Interface";

let n = 0;
const shadow = (o = {}) => Object.assign(
  { type: "outer", color: "1A1730", blur: 9, offset: 2, angle: 90, opacity: 0.1 }, o);

// ---------------------------------------------------------------- helpers
function pageNum(s, dark) {
  n += 1;
  s.addText(String(n).padStart(2, "0"), {
    x: W - M - 0.6, y: H - 0.48, w: 0.6, h: 0.3, align: "right", margin: 0,
    isTextBox: true, fontFace: F, fontSize: 10, color: dark ? "6B6494" : "9AA3B5",
  });
}
function darkSlide() {
  const s = pres.addSlide();
  s.background = { color: C.midnight };
  return s;
}
function lightSlide(eyebrow, title, opts = {}) {
  const s = pres.addSlide();
  s.background = { color: opts.tint ? C.tint : C.paper };
  if (eyebrow) {
    s.addText(eyebrow.toUpperCase(), {
      x: M, y: 0.42, w: CW, h: 0.26, margin: 0, isTextBox: true,
      fontFace: F, fontSize: 11.5, bold: true, color: C.violet, charSpacing: 1.6,
    });
  }
  s.addText(title, {
    x: M, y: 0.72, w: opts.titleW || CW, h: opts.titleH || 0.62, margin: 0, isTextBox: true,
    fontFace: F, fontSize: opts.titleSize || 32, bold: true, color: C.ink,
  });
  return s;
}
function card(s, o) {
  s.addShape(pres.ShapeType.roundRect, {
    x: o.x, y: o.y, w: o.w, h: o.h, rectRadius: 0.07,
    fill: { color: o.fill || C.tint },
    line: { color: o.line || C.line, width: o.lineW || 1 },
    shadow: o.shadow ? shadow() : undefined,
  });
}
function stat(s, o) {
  s.addText(o.value, {
    x: o.x, y: o.y, w: o.w, h: o.vh || 0.82, margin: 0, isTextBox: true,
    fontFace: o.font || F, fontSize: o.size || 40, bold: true,
    color: o.color || C.violet, align: o.align || "left",
  });
  s.addText(o.label, {
    x: o.x, y: o.y + (o.vh || 0.82) - 0.04, w: o.w, h: o.lh || 0.62, margin: 0, isTextBox: true,
    fontFace: F, fontSize: o.labelSize || 12.5, color: o.labelColor || C.muted, align: o.align || "left",
  });
}
function table(s, rows, o) {
  s.addTable(rows, {
    x: o.x, y: o.y, w: o.w, colW: o.colW,
    border: { type: "solid", color: C.line, pt: 1 },
    fontFace: F, fontSize: o.fontSize || 12, color: C.ink,
    valign: "middle", autoPage: false, rowH: o.rowH,
  });
}
const th = (t) => ({ text: t, options: { bold: true, color: C.paper, fill: { color: C.deep }, fontSize: 11.5 } });
const td = (t, opt = {}) => ({ text: t, options: Object.assign({ color: C.ink }, opt) });
const bullets = (items) => items.map((t, j) => ({ text: t, options: { bullet: true, breakLine: j < items.length - 1 } }));

// ================================================================ 01 TITLE
{
  const s = darkSlide();
  s.addShape(pres.ShapeType.roundRect, {
    x: -2.2, y: -2.6, w: 7.2, h: 7.2, rectRadius: 0.5,
    fill: { color: C.violet, transparency: 84 }, line: { type: "none" },
  });
  s.addShape(pres.ShapeType.roundRect, {
    x: 9.4, y: 4.4, w: 6.4, h: 6.4, rectRadius: 0.5,
    fill: { color: C.violet, transparency: 88 }, line: { type: "none" },
  });
  s.addText("SOLANA VALUATION INTERFACE", {
    x: M, y: 1.62, w: CW, h: 0.32, margin: 0, isTextBox: true,
    fontFace: F, fontSize: 13, bold: true, color: C.violetLt, charSpacing: 2.4,
  });
  s.addText("AAPLx trades all night.\nApple doesn't.", {
    x: M, y: 2.08, w: 10.4, h: 2.0, margin: 0, isTextBox: true,
    fontFace: F, fontSize: 50, bold: true, color: C.paper, lineSpacing: 54,
  });
  s.addText([
    { text: "A tokenized stock trades 24/7. The share it represents trades 6.5 hours a day. SVI puts the real value on-chain — ", options: { color: "C9C4E4" } },
    { text: "in an account anyone can read, that tells you when the market is shut.", options: { color: C.paper, bold: true } },
  ], {
    x: M, y: 4.28, w: 10.2, h: 0.9, margin: 0, isTextBox: true,
    fontFace: F, fontSize: 16.5, lineSpacing: 24,
  });
  s.addShape(pres.ShapeType.line, { x: M, y: 5.5, w: 3.0, h: 0, line: { color: C.violet, width: 2.5 } });
  s.addText([
    { text: "Two Pyth feeds in, two verifiable quotes out, one transaction  ·  devnet", options: { bold: true, color: C.paper } },
    { text: "\nPrince  ·  SVI  ·  September 2026", options: { color: "9F98C4" } },
  ], {
    x: M, y: 5.72, w: 8, h: 0.8, margin: 0, isTextBox: true, fontFace: F, fontSize: 14, lineSpacing: 21,
  });
  pageNum(s, true);
  s.addNotes("Say the title out loud and stop. It does the work. Do not explain the product yet -- the next slide is the problem, and the audience has to feel it before the fix means anything.");
}

// ================================================================ 02 THE RECEIPT
{
  const s = lightSlide("Adapter #2 · hylo-xsol-nav-v1", "The interface already generalises. Here is the receipt.");
  s.addText("Before stocks, the same core published a leveraged token's NAV — computed by subtraction from a protocol's vault, nothing like a Pyth feed. 18 Sep 2026, on a fork of Solana mainnet with Hylo's real accounts re-cloned seconds before.", {
    x: M, y: 1.4, w: 11.4, h: 0.4, margin: 0, isTextBox: true, fontFace: F, fontSize: 14, color: C.muted,
  });
  const stats = [
    ["$0.0697", "per xSOL — $0.069715188,\ncomputed on-chain by hylo-core", C.violet],
    ["13.5 bps", "wide band: redeem $0.069715188,\nmint $0.069809602", C.violet],
    ["0", "difference from an independent\noff-chain recomputation", C.green],
    ["~13k CU", "to refresh; ~200 CU\nfor a consumer to read", C.muted],
  ];
  const cw = (CW - 3 * 0.3) / 4;
  stats.forEach(([v, l, col], i) => {
    const x = M + i * (cw + 0.3);
    card(s, { x, y: 2.0, w: cw, h: 1.9, fill: C.paper, line: C.line, shadow: true });
    stat(s, { x: x + 0.28, y: 2.22, w: cw - 0.56, value: v, label: l, size: 36, color: col, labelSize: 11.5 });
  });

  card(s, { x: M, y: 4.16, w: 7.4, h: 2.5, fill: C.deep, line: C.deep });
  s.addText("PUBLISHED QUOTE  ·  2EhQfH26gQBxi8RteUVCXXA5MMnwuLZgFnMqQxLkseuc", {
    x: M + 0.3, y: 4.34, w: 6.9, h: 0.26, margin: 0, isTextBox: true, fontFace: MONO, fontSize: 9.5, color: "9F98C4" });
  s.addText(
    "quote_amount    $ 0.069715188\n" +
    "lower / upper   $ 0.069715188 .. $ 0.069809602\n" +
    "base_amount     1000000  (1 whole xSOL)\n" +
    "observed_slot   448083727   valid_until 448084477\n" +
    "sequence        1           flags  BUY_ZONE\n" +
    "off-chain hylo-core:  $ 0.069715188   MATCH", {
    x: M + 0.3, y: 4.68, w: 6.9, h: 1.86, margin: 0, isTextBox: true, fontFace: MONO, fontSize: 12, color: C.paper, lineSpacing: 18 });

  card(s, { x: M + 7.64, y: 4.16, w: 4.453, h: 2.5, fill: C.greenBg, line: "BFE3CC" });
  s.addText("What this proves", { x: M + 7.94, y: 4.36, w: 3.9, h: 0.32, margin: 0, isTextBox: true,
    fontFace: F, fontSize: 15, bold: true, color: C.green });
  s.addText(bullets([
    "The adapter read Hylo's real state and priced xSOL with Hylo's own library",
    "It called svi-core over a frozen byte format, signed by its own PDA",
    "svi-core wrote the quote; a consumer read it back by byte offset",
    "An off-chain reader over the same bytes agreed to the last digit",
  ]), { x: M + 7.94, y: 4.74, w: 3.9, h: 1.8, margin: 0, isTextBox: true, fontFace: F, fontSize: 11, color: C.ink, lineSpacing: 14.5, paraSpaceAfter: 4 });
  pageNum(s, false);
  s.addNotes("Say the word 'fork' out loud: same bytes as mainnet, local validator. The point of this slide is not xSOL -- it is that two adapters this different publish into one account shape consumers read the same way. That is why it is an interface and not an oracle.");
}

// ================================================================ 03 THE PROBLEM
{
  const s = darkSlide();
  s.addText("THE HOLE IN EVERY TOKENIZED STOCK", {
    x: M, y: 0.62, w: CW, h: 0.3, margin: 0, isTextBox: true,
    fontFace: F, fontSize: 12, bold: true, color: C.violetLt, charSpacing: 2,
  });
  s.addText([
    { text: "For two thirds of every week,\n", options: { color: C.paper } },
    { text: "the token has a price and\nthe company does not.", options: { color: C.violetLt } },
  ], {
    x: M, y: 1.16, w: 11.8, h: 2.5, margin: 0, isTextBox: true,
    fontFace: F, fontSize: 38, bold: true, lineSpacing: 48 });

  s.addText("Thin overnight books. A rumour. One large seller. The token moves — and no market on earth will arbitrage it back, because the thing it tracks is closed.", {
    x: M, y: 3.86, w: 11.4, h: 0.8, margin: 0, isTextBox: true,
    fontFace: F, fontSize: 15, color: "B4ADD4", lineSpacing: 22 });

  const bx = [
    ["Nothing on-chain says so.", "The account holds a number. It carries no signal that the number is a guess made in an empty room."],
    ["So a lender liquidates against it.", "It cannot tell a price from a guess. By the opening bell the price is back; the liquidation is not."],
    ["And this is the fastest-growing asset class on Solana.", "Every tokenized equity has the same hole, and every venue that prices one inherits it."],
  ];
  bx.forEach(([h, b], i) => {
    const x = M + i * (3.93 + 0.29);
    card(s, { x, y: 5.02, w: 3.93, h: 1.62, fill: C.deep, line: "3B3266" });
    s.addText(h, { x: x + 0.24, y: 5.2, w: 3.45, h: 0.5, margin: 0, isTextBox: true, fontFace: F, fontSize: 13.5, bold: true, color: C.paper });
    s.addText(b, { x: x + 0.24, y: 5.72, w: 3.45, h: 0.8, margin: 0, isTextBox: true, fontFace: F, fontSize: 11.5, color: "B4ADD4", lineSpacing: 15 });
  });
  pageNum(s, true);
  s.addNotes("Do not rush this slide. The whole pitch rests on the audience agreeing that a 3am price on a closed stock is not a price. If they nod here, everything after is mechanics.");
}

// ================================================================ 03b WHAT GETS PUBLISHED
{
  const s = lightSlide("The fix", "Two numbers per stock, written in one transaction");
  s.addText("Both quotes come out of the same instruction, so the pair can never be half-updated — there is no moment at which a fresh market price sits against a stale fair value.", {
    x: M, y: 1.4, w: 11.4, h: 0.4, margin: 0, isTextBox: true, fontFace: F, fontSize: 14, color: C.muted });

  const feeds = [
    ["FAIR VALUE", "stock-aapl-fair-value-v1", "What the real share is worth", "Pyth  Equity.US.AAPL/USD", C.violet],
    ["MARKET PRICE", "stock-aapl-market-v1", "What the token trades at on Solana", "Pyth  Crypto.AAPLX/USD", C.amber],
  ];
  feeds.forEach(([tag, feed, what, src, col], i) => {
    const x = M + i * (5.9 + 0.29);
    card(s, { x, y: 2.0, w: 5.9, h: 1.72, fill: C.paper, line: C.line, shadow: true });
    s.addText(tag, { x: x + 0.28, y: 2.2, w: 5.3, h: 0.28, margin: 0, isTextBox: true, fontFace: F, fontSize: 11.5, bold: true, color: col, charSpacing: 1.8 });
    s.addText(what, { x: x + 0.28, y: 2.5, w: 5.3, h: 0.36, margin: 0, isTextBox: true, fontFace: F, fontSize: 18, bold: true, color: C.ink });
    s.addText(feed, { x: x + 0.28, y: 2.92, w: 5.3, h: 0.26, margin: 0, isTextBox: true, fontFace: MONO, fontSize: 11, color: C.muted });
    s.addText(src, { x: x + 0.28, y: 3.24, w: 5.3, h: 0.26, margin: 0, isTextBox: true, fontFace: MONO, fontSize: 11, color: C.muted });
  });

  s.addText("The flags are the product", {
    x: M, y: 3.96, w: 6, h: 0.36, margin: 0, isTextBox: true, fontFace: F, fontSize: 17, bold: true, color: C.ink });
  s.addText("The value is the easy part. What is worth deploying is an account that says what kind of moment it was taken in.", {
    x: M, y: 4.32, w: 11.4, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 13, color: C.muted });

  const flags = [
    ["MARKET_CLOSED", "equity price > 15 min old — impossible mid-session, so the exchange is shut", C.amber],
    ["DEVIATION_HIGH", "the token is more than 2% from the real share", C.amber],
    ["REFERENCE_STALE", "> 4 days, which no weekend or holiday explains", C.muted],
    ["8 days → REFUSE", "not a flag. Nothing is published and the last quote expires", C.red],
  ];
  const fw = (CW - 3 * 0.26) / 4;
  flags.forEach(([n, d, col], i) => {
    const x = M + i * (fw + 0.26);
    card(s, { x, y: 4.78, w: fw, h: 1.5, fill: C.paper, line: C.line, shadow: true });
    s.addText(n, { x: x + 0.22, y: 4.98, w: fw - 0.44, h: 0.3, margin: 0, isTextBox: true, fontFace: MONO, fontSize: 11.5, bold: true, color: col });
    s.addText(d, { x: x + 0.22, y: 5.32, w: fw - 0.44, h: 0.86, margin: 0, isTextBox: true, fontFace: F, fontSize: 11, color: C.ink, lineSpacing: 14.5 });
  });

  s.addText("Fail stale, never fail wrong. An expired quote stops a consumer; a plausible wrong one does not.", {
    x: M, y: 6.42, w: 11.4, h: 0.3, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5, italic: true, color: C.muted });
  pageNum(s, false);
  s.addNotes("MARKET_CLOSED is not an error and saying so is the point. An equity feed is SUPPOSED to be hours old overnight. If we refused to publish then, the feed would go dark exactly when a consumer most needs telling.");
}

// ================================================================ 04 TRUST CHAIN
{
  const s = pres.addSlide();
  s.background = { color: C.paper };
  s.addImage({ path: path.join(DIA, "svi-trust-chain.png"), x: 0.42, y: 0.0, w: 12.5, h: 7.5 });
  pageNum(s, false);
  s.addNotes("Left is the pipeline Pyth asked Hylo for. Right is the same pipeline with the arithmetic moved on-chain. Be explicit that the publisher key on the right is still trusted — we do not fix that, and saying so buys credibility for everything else.");
}

// ================================================================ 05 EVIDENCE
{
  const s = lightSlide("Evidence", "This failure class already has a body count");
  s.addText("In every case below, no on-chain account was ever wrong. The loss came entirely from the valuation layer between true state and the consumer.", {
    x: M, y: 1.4, w: 11.2, h: 0.4, margin: 0, isTextBox: true, fontFace: F, fontSize: 14, color: C.muted });
  const inc = [
    ["$5.8M", "Loopscale", "Apr 2025 · Solana", "The lender's own pricing of RateX PT tokens was wrong. Undercollateralised loans drained 12% of TVL — two weeks after launch.", C.red],
    ["$1.8M", "Moonwell", "2025 · bad debt", "A config change priced cbETH via cbETH/ETH only, dropping the ETH/USD leg. It read $1.12 instead of ~$2,200. Bots did the rest.", C.red],
    ["1.06→1.7", "wUSDM / Venus", "Feb 2026", "A donation attack moved the ERC-4626 share exchange rate itself, then self-liquidated against it. The rate was the attack surface.", C.amber],
    ["$10.2M", "YieldBlox", "Feb 2026", "Thin-liquidity VWAP oracle on a tokenised-treasury pair inflated collateral value enough to over-borrow.", C.red],
  ];
  inc.forEach(([v, t, d, b, col], i) => {
    const x = M + i * (2.8 + 0.31);
    card(s, { x, y: 2.02, w: 2.8, h: 3.06, fill: C.paper, line: C.line, shadow: true });
    s.addText(v, { x: x + 0.22, y: 2.24, w: 2.36, h: 0.56, margin: 0, isTextBox: true, fontFace: F, fontSize: v.length > 6 ? 24 : 30, bold: true, color: col });
    s.addText(t, { x: x + 0.22, y: 2.86, w: 2.36, h: 0.3, margin: 0, isTextBox: true, fontFace: F, fontSize: 15, bold: true, color: C.ink });
    s.addText(d, { x: x + 0.22, y: 3.16, w: 2.36, h: 0.26, margin: 0, isTextBox: true, fontFace: F, fontSize: 10.5, color: C.muted });
    s.addText(b, { x: x + 0.22, y: 3.5, w: 2.36, h: 1.44, margin: 0, isTextBox: true, fontFace: F, fontSize: 11, color: C.ink, lineSpacing: 15 });
  });
  card(s, { x: M, y: 5.36, w: CW, h: 1.28, fill: C.redBg, line: "F5C2C2" });
  s.addText("$137M+", { x: M + 0.34, y: 5.6, w: 1.9, h: 0.5, margin: 0, isTextBox: true, fontFace: F, fontSize: 27, bold: true, color: C.red });
  s.addText("lost across DeFi exploits in Q1 2026 alone; over $1B cumulatively in 2026.", {
    x: M + 2.3, y: 5.66, w: 5.6, h: 0.6, margin: 0, isTextBox: true, fontFace: F, fontSize: 13.5, color: C.ink, lineSpacing: 18 });
  s.addText("Loopscale lists hyUSD today. A lender that hand-rolls valuation for every protocol asset it accepts will eventually hand-roll one wrong.", {
    x: M + 8.1, y: 5.6, w: 3.6, h: 0.86, margin: 0, isTextBox: true, fontFace: F, fontSize: 11.5, italic: true, color: C.muted, lineSpacing: 15 });
  pageNum(s, false);
  s.addNotes("Same class, not the same protocol. Loopscale is the sharpest: on Solana, and lists hyUSD.");
}

// ================================================================ 06 THE IDEA
{
  const s = darkSlide();
  s.addText("THE IDEA", { x: M, y: 1.5, w: CW, h: 0.3, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5, bold: true, color: C.violetLt, charSpacing: 2.2 });
  s.addText([
    { text: "Move the arithmetic on-chain.\n", options: { color: C.paper } },
    { text: "Then RPCs, APIs and keys become couriers —\nnot authors — of the number.", options: { color: "A79FD0", fontSize: 30 } },
  ], { x: M, y: 2.0, w: 11.8, h: 2.6, margin: 0, isTextBox: true, fontFace: F, fontSize: 40, bold: true, lineSpacing: 50 });
  const pts = [
    ["Computed", "by a Solana program, from accounts it verifies itself, with the protocol's own math library"],
    ["Published", "to a 320-byte account with a fixed layout that anyone can read for ~200 CU"],
    ["Reproducible", "byte-for-byte by any third party over the same accounts at the same slot"],
  ];
  pts.forEach(([h, b], i) => {
    const x = M + i * (3.93 + 0.29);
    s.addShape(pres.ShapeType.line, { x, y: 5.05, w: 3.93, h: 0, line: { color: C.violet, width: 2.5 } });
    s.addText(h, { x, y: 5.2, w: 3.93, h: 0.36, margin: 0, isTextBox: true, fontFace: F, fontSize: 17, bold: true, color: C.paper });
    s.addText(b, { x, y: 5.58, w: 3.7, h: 0.7, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5, color: "9F98C4", lineSpacing: 17 });
  });
  pageNum(s, true);
}

// ================================================================ 07 ARCHITECTURE
{
  const s = pres.addSlide();
  s.background = { color: C.paper };
  s.addImage({ path: path.join(DIA, "svi-architecture.png"), x: 0.73, y: 0.0, w: 11.88, h: 7.5 });
  pageNum(s, false);
  s.addNotes("Grey = exists already, we touch nothing. Purple = we deploy. Amber = the product. Green = operations. Blue = consumers. The keeper pays gas but cannot influence the value. Hylo's programs are never modified and never asked for permission: Solana account data is public bytes.");
}

// ================================================================ 08 WHERE PYTH SITS
{
  const s = lightSlide("The question everyone asks", "Do we depend on Pyth, or does Pyth depend on us?");
  card(s, { x: M, y: 1.5, w: 5.94, h: 2.9, fill: C.paper, line: C.line, shadow: true });
  s.addText("Today: we depend on Pyth", { x: M + 0.32, y: 1.72, w: 5.3, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 16.5, bold: true, color: C.ink });
  s.addText("Pyth answers one question — what is 1 SOL worth right now — from exchanges. xSOL's value is calculated from that answer plus Hylo's books: collateral × SOL price, minus hyUSD debt, divided by supply. Pyth is our input. Nobody else can be.\n\nSo our quote can never be fresher than the SOL price it came from. Hylo's own rule is that a price older than 10 seconds is unusable; the adapter inherits that rule unchanged and refuses rather than publish from a stale input.", {
    x: M + 0.32, y: 2.12, w: 5.3, h: 2.2, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, color: C.muted, lineSpacing: 16.5 });

  card(s, { x: M + 6.16, y: 1.5, w: 5.94, h: 2.9, fill: C.paper, line: C.line, shadow: true });
  s.addText("Later: Pyth could depend on us", { x: M + 6.48, y: 1.72, w: 5.3, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 16.5, bold: true, color: C.ink });
  s.addText("Nobody publishes the output — xSOL's NAV — on-chain. Once SVI does, an oracle network can consume that account to offer an xSOL/USD feed, the way it wraps other on-chain sources, instead of asking each protocol to run a server and an API.\n\nThat is the request in Plish's post, answered without the security hole: the oracle reads a public account whose contents anyone can recompute.", {
    x: M + 6.48, y: 2.12, w: 5.3, h: 2.2, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, color: C.muted, lineSpacing: 16.5 });

  card(s, { x: M, y: 4.66, w: CW, h: 2.0, fill: C.deep, line: C.deep });
  s.addText("The claim the demo supports", { x: M + 0.32, y: 4.86, w: 6, h: 0.32, margin: 0, isTextBox: true, fontFace: F, fontSize: 15, bold: true, color: C.paper });
  s.addText([
    { text: "Not ", options: { color: "B4ADD4" } },
    { text: "“fresher than Pyth”", options: { color: C.red, bold: true } },
    { text: ". It is: ", options: { color: "B4ADD4" } },
    { text: "the number nobody publishes, now on-chain, moving 2.5× SOL, with bounds and a staleness horizon, written only by code.", options: { color: C.paper, bold: true } },
  ], { x: M + 0.32, y: 5.26, w: CW - 0.64, h: 1.2, margin: 0, isTextBox: true, fontFace: F, fontSize: 16, lineSpacing: 23 });
  pageNum(s, false);
  s.addNotes("This slide exists because the first framing of the video was 'our price is fresher than Pyth', which is false and would be dismantled in the replies. Pyth publishes the input; SVI publishes the output.");
}

// ================================================================ 09 THE FIVE PIECES
{
  const s = lightSlide("Architecture", "Five pieces, and what each one is for");
  const rows = [
    ["1", "Adapter", "the calculator", "One per protocol. Reads the protocol's public account bytes, verifies owner / PDA / mint / oracle, and calls the protocol's own math library. Reimplements nothing.", C.violet],
    ["2", "Core", "the notice board", "Tiny, protocol-agnostic, no arithmetic. Only the adapter PDA named in the feed descriptor may write. Sequence monotonic, validity stamped, bounds checked.", C.violet],
    ["3", "Quote account", "the product", "320 fixed bytes: value, lower/upper bounds, observed slot, valid_until, sequence, status flags, methodology hash. Read for ~200 CU.", C.amber],
    ["4", "Keeper", "rings the bell", "Permissionless — anyone can crank. Pays gas; cannot influence the value. If nobody cranks, the quote expires visibly. Built; ships with a watcher.", C.green],
    ["5", "Observer", "the second accountant", "Recomputes from raw accounts through independent RPCs. A divergence of one unit alerts and halts the mirror. Next.", C.green],
  ];
  let y = 1.52;
  rows.forEach(([num, name, tag, body, col]) => {
    s.addShape(pres.ShapeType.ellipse, { x: M, y: y + 0.06, w: 0.52, h: 0.52, fill: { color: col } });
    s.addText(num, { x: M, y: y + 0.14, w: 0.52, h: 0.38, margin: 0, isTextBox: true, align: "center", fontFace: F, fontSize: 17, bold: true, color: C.paper });
    s.addText([
      { text: name, options: { bold: true, fontSize: 16, color: C.ink } },
      { text: "   " + tag, options: { fontSize: 12.5, color: col, italic: true } },
    ], { x: M + 0.74, y: y + 0.04, w: 3.4, h: 0.36, margin: 0, isTextBox: true, fontFace: F });
    s.addText(body, { x: M + 4.0, y: y - 0.01, w: 8.1, h: 0.72, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, color: C.muted, lineSpacing: 16 });
    y += 0.9;
    if (num !== "5") s.addShape(pres.ShapeType.line, { x: M, y: y - 0.11, w: CW, h: 0, line: { color: C.line, width: 1 } });
  });
  card(s, { x: M, y: 6.14, w: CW, h: 0.72, fill: C.tint, line: C.line });
  s.addText([
    { text: "The principle it all reduces to:  ", options: { bold: true, color: C.ink } },
    { text: "there is no point in the pipeline where a human or a hackable server chooses the number.", options: { color: C.muted } },
  ], { x: M + 0.3, y: 6.32, w: CW - 0.6, h: 0.4, margin: 0, isTextBox: true, fontFace: F, fontSize: 13.5 });
  pageNum(s, false);
}

// ================================================================ 10 ONE REFRESH
{
  const s = lightSlide("Mechanics", "One transaction, every read from the same slot");
  const steps = [
    ["Crank", "Anyone sends refresh_xsol_nav, passing Hylo's state, the xSOL mint, Pyth SOL/USD and the quote account."],
    ["Verify", "Owner, canonical address, mint, and that the Pyth account is the one Hylo's own state names. Any mismatch aborts."],
    ["Check", "hylo-core's five gates: cache epoch, Pyth verification, publish time, slot, confidence. Each failure has its own code."],
    ["Compute", "Redeem NAV (floor, lower price) is the value and lower bound; mint NAV (ceil, upper price) is the upper bound."],
    ["Publish", "Band ≤ 500 bps, bounds coherent, flags set honestly. CPI into svi-core signed by the adapter PDA. Sequence + 1."],
  ];
  const cw = (CW - 4 * 0.22) / 5;
  steps.forEach(([h, b], i) => {
    const x = M + i * (cw + 0.22);
    card(s, { x, y: 1.6, w: cw, h: 2.5, fill: C.paper, line: C.line, shadow: true });
    s.addText(String(i + 1), { x: x + 0.2, y: 1.76, w: 0.8, h: 0.5, margin: 0, isTextBox: true, fontFace: F, fontSize: 30, bold: true, color: C.violetLt });
    s.addText(h, { x: x + 0.2, y: 2.3, w: cw - 0.4, h: 0.32, margin: 0, isTextBox: true, fontFace: F, fontSize: 15.5, bold: true, color: C.ink });
    s.addText(b, { x: x + 0.2, y: 2.66, w: cw - 0.4, h: 1.3, margin: 0, isTextBox: true, fontFace: F, fontSize: 11, color: C.muted, lineSpacing: 15 });
    if (i < 4) s.addText("→", { x: x + cw + 0.005, y: 2.62, w: 0.21, h: 0.3, margin: 0, isTextBox: true, align: "center", fontFace: F, fontSize: 15, bold: true, color: C.violetLt });
  });
  card(s, { x: M, y: 4.34, w: 5.94, h: 2.28, fill: C.greenBg, line: "BFE3CC" });
  s.addText("Permissionless, yet uninfluenceable", { x: M + 0.3, y: 4.56, w: 5.34, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 16, bold: true, color: C.green });
  s.addText("Anyone can send this transaction. Nobody can change what it writes — the value is computed by adapter code from verified accounts, and only the adapter's PDA can authorise the write. The keeper is paying for gas. If our bot dies, anyone else's bot cranks it.", {
    x: M + 0.3, y: 4.96, w: 5.34, h: 1.5, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5, color: C.ink, lineSpacing: 17 });
  card(s, { x: M + 6.16, y: 4.34, w: 5.94, h: 2.28, fill: C.amberBg, line: "F0D9A0" });
  s.addText("We reimplement none of Hylo's math", { x: M + 6.46, y: 4.56, w: 5.34, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 16, bold: true, color: C.amber });
  s.addText("The adapter links hylo-core — the library Hylo's own program uses — pinned to one commit, recorded on-chain in the adapter config. levercoin_redeem_nav and levercoin_mint_nav are called, not copied. Hylo's redemption math and SVI's published value cannot diverge, because they are the same code.", {
    x: M + 6.46, y: 4.96, w: 5.34, h: 1.5, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5, color: C.ink, lineSpacing: 17 });
  pageNum(s, false);
}

// ================================================================ 11 FAIL STALE
{
  const s = darkSlide();
  s.addText("THE DESIGN RULE", { x: M, y: 0.56, w: CW, h: 0.3, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, bold: true, color: C.violetLt, charSpacing: 2 });
  s.addText("Fail stale, never fail wrong.", { x: M, y: 0.92, w: CW, h: 0.78, margin: 0, isTextBox: true, fontFace: F, fontSize: 40, bold: true, color: C.paper });
  s.addText("No code path may publish a value computed from unvalidated inputs. A visibly expired quote is always preferable to a plausible wrong one.", {
    x: M, y: 1.76, w: 10.6, h: 0.4, margin: 0, isTextBox: true, fontFace: F, fontSize: 14.5, color: "A79FD0" });
  const rows = [
    [th("Condition"), th("Behaviour"), th("What the consumer sees")],
    [td("Everything healthy"), td("Publish, sequence + 1"), td("Fresh quote, flags clear")],
    [td("Hylo's collateral cache is from a past epoch"), td("Abort — no write  (HyloCacheStale)", { bold: true, color: C.red }), td("Quote ages past valid_until_slot")],
    [td("Pyth price older than Hylo's 10 s window"), td("Abort — no write  (OracleStale)", { bold: true, color: C.red }), td("Quote ages past valid_until_slot")],
    [td("Pyth confidence wider than Hylo tolerates"), td("Abort — no write  (OracleConfidenceTooWide)", { bold: true, color: C.red }), td("Quote ages past valid_until_slot")],
    [td("Mint/redeem band over 500 bps"), td("Abort — no write  (BandTooWide)", { bold: true, color: C.red }), td("Quote ages past valid_until_slot")],
    [td("Hylo underwater (collateral ratio < 100%)"), td("Publish exactly 0 with halt flags", { bold: true, color: C.amber }), td("DESTABILIZED — collateral value zero, not a dip to buy")],
    [td("xSOL supply is 0"), td("Publish 1.000000000, flagged"), td("ZERO_SUPPLY_DEFAULT")],
    [td("Wrong Pyth account passed by a keeper"), td("Abort — no write  (OracleMismatch)", { bold: true, color: C.red }), td("Nothing; the keeper was wrong or malicious")],
  ];
  s.addTable(rows, {
    x: M, y: 2.34, w: CW, colW: [3.9, 4.4, 3.793],
    border: { type: "solid", color: "3B3266", pt: 1 }, fill: { color: "FFFFFF" },
    fontFace: F, fontSize: 11.5, color: C.ink, valign: "middle", rowH: 0.42,
  });
  s.addText("Every row above is implemented and tested. SVI imposes no oracle tolerance of its own — it reads Hylo's on-chain OracleConfig on every refresh, so the feed reflects exactly what the protocol would accept for a real redemption.", {
    x: M, y: 6.62, w: CW, h: 0.44, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, italic: true, color: "9F98C4" });
  pageNum(s, true);
  s.addNotes("The slide a risk person cares about most. Every failure mode is visible and none produces a plausible wrong number. Each refusal logs the epochs, slots and timestamps behind it.");
}

// ================================================================ 12 CONSUMPTION
{
  const s = lightSlide("Integration", "Three ways to consume it, one of them dominant");
  const modes = [
    ["A", "Read the account", "99% of usage", C.blue, C.blueBg,
      ["Pass the quote account into your own instruction", "Check valid_until_slot and status_flags", "Check value_type is what you onboarded", "Use the lower bound for collateral"],
      "One account read, ~200 CU. Fixed 320-byte layout, asserted by a test."],
    ["B", "Call the calculator live", "high-stakes only", C.violet, "EFE9FE",
      ["Include the adapter's compute in your own transaction", "Value derived from this exact slot's state, atomically", "More accounts, ~13k compute units", "Zero staleness"],
      "For liquidators who cannot accept a five-minute-old quote. Most integrators never need this."],
    ["C", "The verified mirror", "off-chain consumers", C.green, C.greenBg,
      ["Fetch the account through two independent RPCs", "Require byte-for-byte agreement", "Decode and serve JSON with full provenance", "Serve an error when stale — never a last-known-good value"],
      "A photocopier. It cannot lie about the price, because anyone can compare its output to the account. Not built yet."],
  ];
  const cw = (CW - 2 * 0.3) / 3;
  modes.forEach(([k, title, tag, col, bg, items, foot], i) => {
    const x = M + i * (cw + 0.3);
    card(s, { x, y: 1.56, w: cw, h: 4.94, fill: C.paper, line: C.line, shadow: true });
    s.addShape(pres.ShapeType.roundRect, { x, y: 1.56, w: cw, h: 0.94, rectRadius: 0.07, fill: { color: bg }, line: { type: "none" } });
    s.addShape(pres.ShapeType.ellipse, { x: x + 0.24, y: 1.79, w: 0.48, h: 0.48, fill: { color: col } });
    s.addText(k, { x: x + 0.24, y: 1.86, w: 0.48, h: 0.36, margin: 0, isTextBox: true, align: "center", fontFace: F, fontSize: 17, bold: true, color: C.paper });
    s.addText(title, { x: x + 0.84, y: 1.78, w: cw - 1.05, h: 0.32, margin: 0, isTextBox: true, fontFace: F, fontSize: 15.5, bold: true, color: C.ink });
    s.addText(tag, { x: x + 0.84, y: 2.1, w: cw - 1.05, h: 0.26, margin: 0, isTextBox: true, fontFace: F, fontSize: 11.5, color: col, bold: true });
    s.addText(bullets(items), { x: x + 0.28, y: 2.7, w: cw - 0.56, h: 2.3, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, color: C.ink, lineSpacing: 16, paraSpaceAfter: 8 });
    s.addShape(pres.ShapeType.line, { x: x + 0.28, y: 5.22, w: cw - 0.56, h: 0, line: { color: C.line, width: 1 } });
    s.addText(foot, { x: x + 0.28, y: 5.34, w: cw - 0.56, h: 1.0, margin: 0, isTextBox: true, fontFace: F, fontSize: 11.5, italic: true, color: C.muted, lineSpacing: 15 });
  });
  s.addText("A worked example: 100,000 xSOL deposited → the market reads valid_until ≥ now, flags clear, lower bound $0.069715188 → $6,971.52 of collateral → $4,183 borrowable at 60% LTV. When valid_until passes, refuse new borrows; never guess.", {
    x: M, y: 6.64, w: CW, h: 0.42, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, color: C.muted });
  pageNum(s, false);
}

// ================================================================ 13 BUILD STATUS
{
  const s = lightSlide("Status", "What exists, and what is next");
  const rows = [
    [th("Component"), th("State"), th("What it is")],
    [td("programs/svi-core"), td("DONE", { bold: true, color: C.green }), td("Descriptor + 320-byte quote, layout asserted by test. Accepts a write only from the adapter PDA named in the descriptor. Sequence, validity, bounds enforced.")],
    [td("programs/svi-stock-adapter"), td("DONE", { bold: true, color: C.green }), td("Two Pyth feeds in, two quotes out, one instruction. Verifies owner, feed id, verification level, publish time, age and confidence, and refuses on each. Thresholds correctable in place. 26 tests.")],
    [td("programs/svi-hylo-adapter"), td("DONE", { bold: true, color: C.green }), td("Adapter #2: xSOL NAV computed by subtraction from Hylo's vault, hylo-core pinned to a commit. 17 tests. Published a real quote on a mainnet fork on 18 Sep, matched to the ninth decimal by an independent reader.")],
    [td("tools/svi-keeper"), td("DONE", { bold: true, color: C.green }), td("Permissionless crank for both feeds, with decoded refusal codes. Can post its own Pyth updates where a feed is unsponsored, given a Hermes endpoint that serves it. 31 tests.")],
    [td("tools/stock-check + dashboard"), td("DONE", { bold: true, color: C.green }), td("An independent verifier sharing no code with the adapter, and a page that reads the accounts from the browser with no backend and no last-known value.")],
    [td("A price source for the stock feeds"), td("BLOCKED", { bold: true, color: C.red }), td("Measured 18 Sep: Pyth's sponsored equity accounts are 6-34 days stale on mainnet, 78 days on devnet, and two of three xStock accounts do not exist on devnet at all. Hermes now answers 401 without a key. Deployment is four commands; the prices are the gap.")],
    [td("Sample lender · observer · audit"), td("NEXT", { color: C.muted }), td("No consumer reads the feed yet, which is the honest gap. No real collateral against these feeds until a third-party audit.")],
    [td("check-pyth.py"), td("DONE", { bold: true, color: C.green }), td("Asks a cluster whether the feeds a deployment needs are fresh, correctly identified and fully verified, and says which of the three answers you have. It is how the row above was measured.")],
  ];
  table(s, rows, { x: M, y: 1.44, w: CW, colW: [3.0, 1.35, 7.743], fontSize: 11, rowH: 0.62 });
  card(s, { x: M, y: 6.5, w: CW, h: 0.6, fill: C.violet, line: C.violet });
  s.addText([
    { text: "The critical path is no longer engineering.  ", options: { color: C.paper, bold: true } },
    { text: "It is a price source for the stock feeds, and a first consumer. The Hylo feed needs neither.", options: { color: "E4DDFB" } },
  ], { x: M + 0.34, y: 6.64, w: CW - 0.68, h: 0.36, margin: 0, isTextBox: true, fontFace: F, fontSize: 15 });
  pageNum(s, false);
}

// ================================================================ 14 MARKET
{
  const s = lightSlide("Market", "The asset class is NAV-shaped, and it is growing fast");
  const stats = [
    ["$3.7B", "tokenised RWA on Solana,\nup 4× in six months", C.violet],
    ["~$15B", "Solana LST supply —\nevery unit a formula, not a trade", C.violet],
    ["~$3.5B", "lending TVL carrying this\nvaluation risk today", C.violet],
    ["2,800+", "Pyth feeds — built for\nobserved market prices", C.muted],
  ];
  const cw = (CW - 3 * 0.3) / 4;
  stats.forEach(([v, l, col], i) => {
    const x = M + i * (cw + 0.3);
    card(s, { x, y: 1.5, w: cw, h: 1.86, fill: C.paper, line: C.line, shadow: true });
    stat(s, { x: x + 0.28, y: 1.72, w: cw - 0.56, value: v, label: l, size: 38, color: col, labelSize: 12 });
  });
  card(s, { x: M, y: 3.6, w: 5.94, h: 3.06, fill: C.tint, line: C.line });
  s.addText("Why now", { x: M + 0.32, y: 3.82, w: 5.3, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 17, bold: true, color: C.ink });
  s.addText(bullets([
    "A protocol founder asked for exactly this, in public, in August — because Pyth is asking him for an API.",
    "Hylo's next version multiplies 4 feeds into 4 × N assets.",
    "Solana RWA quadrupled in six months, and it is NAV-priced by definition.",
    "The Foundation is standardising the neighbouring layer (sRFC 40) right now.",
    "Colosseum runs 28 Sep – 2 Nov 2026. The first feed already works.",
  ]), { x: M + 0.32, y: 4.24, w: 5.3, h: 2.2, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5, color: C.ink, lineSpacing: 16, paraSpaceAfter: 7 });
  card(s, { x: M + 6.16, y: 3.6, w: 5.94, h: 3.06, fill: C.deep, line: C.deep });
  s.addText("The honest version", { x: M + 6.48, y: 3.82, w: 5.3, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 17, bold: true, color: C.paper });
  s.addText("Standards do not monetise. Realistic year-one revenue is low six figures at best, from services beside a permanently free standard — custom adapters, managed keepers, reviews. There is no token and there will not be one.\n\nThe return is position: authoring the valuation interface Solana adopts, with live feeds, while the Foundation standardises next door. Cost to find out: about 3.3 SOL to deploy and $50–150 a month to run.", {
    x: M + 6.48, y: 4.24, w: 5.3, h: 2.2, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, color: "B4ADD4", lineSpacing: 16 });
  pageNum(s, false);
  s.addNotes("Sources: Solana RWA $3.7B (Jul 2026); LST supply approaching $15B (May 2026, Sanctum); lending TVL Kamino ~$2.1B + marginfi ~$700M + Save ~$400M + Drift spot ~$300M. Say the honest-version box out loud — it is what makes the rest believable.");
}

// ================================================================ 15 LANDSCAPE
{
  const s = lightSlide("Landscape", "Nobody does this as an open interface");
  const rows = [
    [th("Player"), th("What it is"), th("Why SVI is not redundant")],
    [td("Pyth", { bold: true }), td("Push/pull oracle, 2,800+ feeds, 128 publishers"), td("Upstream input and prospective consumer, not competitor. Built for observed market prices; it has no mechanism for reproducible protocol NAV, which is why it asks protocols for APIs.")],
    [td("Switchboard", { bold: true }), td("Permissionless custom feeds, TEE-executed jobs"), td("Real overlap, but the math still runs off-chain in an enclave. You trade 'trust the API' for 'trust the enclave and the job definition'.")],
    [td("Kamino Scope", { bold: true }), td("Oracle aggregator, ~40 asset-specific valuation types"), td("Closest analogue and proof the adapter model works — but it is Kamino's internal component, curated by and for Kamino. A consumer target, not a rival.")],
    [td("Blueshift Doppler", { bold: true }), td("Ultra-efficient price publishing primitive"), td("Transport, not semantics. It makes writing a number cheap; it says nothing about where the number came from.")],
    [td("sRFC 40", { bold: true }), td("Foundation + Exo vault standard, NAV-settled flows"), td("The one genuine strategic risk. Response is to contribute into it, not compete — and to cover what vaults don't: leveraged tokens, backing NAV, redemption, receipt tokens.")],
    [td("Chainlink NAV", { bold: true }), td("Brings off-chain fund NAV on-chain for RWAs"), td("Opposite direction: it transports NAV computed off-chain by a fund administrator. SVI computes NAV from on-chain state.")],
  ];
  table(s, rows, { x: M, y: 1.5, w: CW, colW: [1.85, 3.5, 6.743], fontSize: 11, rowH: 0.66 });
  s.addText("The honest read: nobody does exactly this, and not because it is hard — because it does not obviously monetise. Kamino, Switchboard and Blueshift could each build it in weeks. The defensible assets are methodology specs, audits, track record and integrations — not code. Which is why the code is open.", {
    x: M, y: 6.4, w: CW, h: 0.6, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, italic: true, color: C.muted, lineSpacing: 16 });
  pageNum(s, false);
}

// ================================================================ 16 ROADMAP
{
  const s = lightSlide("Roadmap", "Six stages, each with a gate before the next", { tint: true });
  const stages = [
    ["0", "Prove it on a fork", "Adapter publishes real xSOL NAV; off-chain recomputation matches exactly.", "DONE · 18 Sep", C.green, false, true],
    ["1", "Mainnet + keeper", "Spec frozen and hashed. Both programs deployed. Keeper running; watcher public.", "NOW · ~2 weeks", C.violet, true, false],
    ["2", "Observer + audit", "Zero divergence over 72 h through two RPCs; that run sets the ORACLE_DIVERGENT tolerance. Audit before any real collateral.", "Oct 2026", C.muted, false, false],
    ["3", "First consumer", "One lending market or dashboard reads the quote account in production.", "Q4 2026", C.muted, false, false],
    ["4", "Adapter #2", "A second protocol ships with zero changes to core or the wire format.", "Q1 2027", C.muted, false, false],
    ["5", "Into the sRFC", "Submitted aligned with sRFC 40, not parallel to it.", "Q1–Q2 2027", C.muted, false, false],
  ];
  let y = 1.56;
  stages.forEach(([num, name, body, when, col, now, done]) => {
    if (now) card(s, { x: M - 0.1, y: y - 0.08, w: CW + 0.2, h: 0.78, fill: "EFE9FE", line: C.violetLt });
    s.addShape(pres.ShapeType.ellipse, { x: M, y: y + 0.06, w: 0.5, h: 0.5, fill: { color: done ? C.green : now ? C.violet : "D7D4E4" } });
    s.addText(done ? "✓" : num, { x: M, y: y + 0.14, w: 0.5, h: 0.36, margin: 0, isTextBox: true, align: "center", fontFace: F, fontSize: 16, bold: true, color: (done || now) ? C.paper : C.muted });
    s.addText(name, { x: M + 0.72, y: y + 0.14, w: 2.6, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 15.5, bold: true, color: C.ink });
    s.addText(body, { x: M + 3.42, y: y + 0.16, w: 6.4, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, color: C.muted });
    s.addText(when, { x: M + 9.9, y: y + 0.14, w: 2.19, h: 0.34, margin: 0, isTextBox: true, align: "right", fontFace: F, fontSize: 12.5, bold: true, color: col });
    y += 0.79;
  });
  card(s, { x: M, y: 6.42, w: CW, h: 0.66, fill: C.paper, line: C.line });
  s.addText([
    { text: "Stages 0–2 are within our control and only prove competence.  ", options: { color: C.ink, bold: true } },
    { text: "Stages 3 and 4 are the real test — until a second protocol adopts it, SVI is a Hylo wrapper, and we say so.", options: { color: C.muted } },
  ], { x: M + 0.3, y: 6.58, w: CW - 0.6, h: 0.36, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5 });
  pageNum(s, false);
}

// ================================================================ 17 THE ASK
{
  const s = darkSlide();
  s.addText("THE ASK", { x: M, y: 0.56, w: CW, h: 0.3, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, bold: true, color: C.violetLt, charSpacing: 2 });
  s.addText("Three things, from three audiences", { x: M, y: 0.92, w: CW, h: 0.7, margin: 0, isTextBox: true, fontFace: F, fontSize: 38, bold: true, color: C.paper });
  const asks = [
    ["Superteam", "An instagrant", "Covers the audit, mainnet deployment, and three months of keeper and observer operation. Everything it funds is open source and permanently free.", C.violet],
    ["Lenders & dashboards", "One integration", "Read one 320-byte account. Kamino, marginfi, Loopscale, or any wallet that shows xSOL. A pilot on devnet first; mainnet after the audit.", C.blue],
    ["Hylo", "Fifteen minutes", "Read §§2–6 of the spec and tell us if we misread your math. No program changes, no key, no engineering time. The adapter runs whether or not you answer.", C.green],
  ];
  const cw = (CW - 2 * 0.3) / 3;
  asks.forEach(([who, what, body, col], i) => {
    const x = M + i * (cw + 0.3);
    card(s, { x, y: 1.9, w: cw, h: 3.4, fill: C.deep, line: "3B3266" });
    s.addText(who.toUpperCase(), { x: x + 0.3, y: 2.12, w: cw - 0.6, h: 0.28, margin: 0, isTextBox: true, fontFace: F, fontSize: 11.5, bold: true, color: col, charSpacing: 1.6 });
    s.addText(what, { x: x + 0.3, y: 2.46, w: cw - 0.6, h: 0.5, margin: 0, isTextBox: true, fontFace: F, fontSize: 22, bold: true, color: C.paper });
    s.addText(body, { x: x + 0.3, y: 3.06, w: cw - 0.6, h: 2.1, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5, color: "B4ADD4", lineSpacing: 17.5 });
  });
  card(s, { x: M, y: 5.56, w: CW, h: 1.5, fill: C.midnight, line: "3B3266" });
  s.addText("NOT ASKED FOR", { x: M + 0.32, y: 5.74, w: 3, h: 0.28, margin: 0, isTextBox: true, fontFace: F, fontSize: 11.5, bold: true, color: C.red, charSpacing: 1.6 });
  s.addText("A token  ·  Exclusivity  ·  Hylo's engineering time or permission  ·  Anyone to trust our server — there is no server in the loop", {
    x: M + 0.32, y: 6.08, w: CW - 0.64, h: 0.4, margin: 0, isTextBox: true, fontFace: F, fontSize: 14, color: C.paper });
  s.addText("Colosseum runs 28 Sep – 2 Nov 2026. A mainnet-live feed with a keeper and an observer is the submission.", {
    x: M + 0.32, y: 6.5, w: CW - 0.64, h: 0.36, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5, color: "9F98C4" });
  pageNum(s, true);
  s.addNotes("The Hylo column is an open door, not a dependency. The old deck's five questions still stand in the spec's §10; if Hylo answers them, good; if not, the adapter reads public bytes and mainnet happens anyway.");
}

// ================================================================ 18 CANDOUR
{
  const s = lightSlide("Candour", "The objections worth raising, answered", { tint: true });
  const objs = [
    ["“Doesn't Pyth already do this?”",
     "For stocks it nearly does: Pyth publishes the share and the token as two unrelated feeds. Nobody publishes the relationship, and nobody refuses — Pyth will serve a 34-day-old AAPL price without complaint. For xSOL there is no feed at all: its NAV is computed from Hylo's own reserves, and that number exists nowhere else. We are a valuation layer, not a second price oracle."],
    ["“What if the adapter is wrong and someone gets liquidated?”",
     "A real risk. Hence: call hylo-core rather than reimplement it; refuse rather than publish on any unverified input; an observer that halts on a one-unit divergence; and no real collateral until a third-party audit. Until that audit, our own position is that no market should underwrite against these feeds."],
    ["“Is this a hackathon project that dies in November?”",
     "The mitigations hold regardless: everything open source, the keeper permissionless, the core small enough to freeze, and the methodology spec standing on its own as documentation of how xSOL is valued. If every line of SVI code were abandoned, the quote account keeps working until nobody cranks it — visibly."],
    ["“Where does your price come from, and what when it stops?”",
     "It has stopped. Pyth's sponsored stock accounts are 6-34 days stale on mainnet and mostly absent on devnet; Hermes now needs a key. So the stock feeds need a price source we do not yet have, and we say so on the status slide. The xSOL feed is unaffected — it reads Hylo's accounts directly. Our answer to a dead feed is the product: refuse, visibly, rather than publish the last thing we saw."],
  ];
  const cw = (CW - 0.3) / 2, ch = 2.44;
  objs.forEach(([q, a], i) => {
    const x = M + (i % 2) * (cw + 0.3);
    const y = 1.56 + Math.floor(i / 2) * (ch + 0.28);
    card(s, { x, y, w: cw, h: ch, fill: C.paper, line: C.line, shadow: true });
    s.addText(q, { x: x + 0.3, y: y + 0.24, w: cw - 0.6, h: 0.62, margin: 0, isTextBox: true, fontFace: F, fontSize: 14.5, bold: true, color: C.ink, lineSpacing: 19 });
    s.addText(a, { x: x + 0.3, y: y + 0.88, w: cw - 0.6, h: 1.4, margin: 0, isTextBox: true, fontFace: F, fontSize: 11.5, color: C.muted, lineSpacing: 16 });
  });
  pageNum(s, false);
}

// ================================================================ 19 CLOSE
{
  const s = darkSlide();
  s.addShape(pres.ShapeType.roundRect, { x: 8.6, y: -1.9, w: 7.4, h: 7.4, rectRadius: 0.5, fill: { color: C.violet, transparency: 86 }, line: { type: "none" } });
  s.addText("WHAT HAPPENS NEXT", { x: M, y: 0.86, w: CW, h: 0.3, margin: 0, isTextBox: true, fontFace: F, fontSize: 12, bold: true, color: C.violetLt, charSpacing: 2 });
  const steps = [
    ["Week 1", "Freeze hylo-xsol-nav-v1 at v1.0; publish the hash; deploy both programs to mainnet."],
    ["Week 2", "Keeper live. Quote account address published. Watcher public so anyone can see it tick."],
    ["Week 3–4", "Observer at zero divergence over 72 hours. Audit scoped."],
    ["October", "Colosseum submission: a live feed, a keeper, an observer, and a spec."],
    ["Then", "First consumer on devnet. Second adapter. Into the sRFC."],
  ];
  let y = 1.44;
  steps.forEach(([who, what]) => {
    s.addText(who, { x: M, y: y, w: 1.5, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 14, bold: true, color: C.violetLt });
    s.addText(what, { x: M + 1.6, y: y, w: 8.4, h: 0.34, margin: 0, isTextBox: true, fontFace: F, fontSize: 14, color: C.paper });
    y += 0.56;
  });
  s.addShape(pres.ShapeType.line, { x: M, y: 4.5, w: 9.64, h: 0, line: { color: "342C5C", width: 1 } });
  s.addText("Everything in this deck is in the repository: the programs, the tests, the keeper, the spec, the validation record with the transaction signature, and the page that explains how the number is made.", {
    x: M, y: 4.72, w: 9.64, h: 0.9, margin: 0, isTextBox: true, fontFace: F, fontSize: 14, color: "A79FD0", lineSpacing: 20 });
  s.addText("Fail stale, never fail wrong.", { x: M, y: 5.9, w: 7, h: 0.44, margin: 0, isTextBox: true, fontFace: F, fontSize: 21, bold: true, color: C.paper });
  s.addText("github.com/princedotrs/solana-valuation-interface", { x: M, y: 6.38, w: 8, h: 0.3, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5, color: "7F77AC" });
  s.addText("Prince  ·  @princedotrs", { x: M, y: 6.7, w: 8, h: 0.3, margin: 0, isTextBox: true, fontFace: F, fontSize: 12.5, color: "7F77AC" });
  pageNum(s, true);
}

pres.writeFile({ fileName: OUT }).then(() => console.log("wrote " + OUT));

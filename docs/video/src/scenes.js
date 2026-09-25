// Scene definitions for both videos. `s` = narration sentences (caption text).
// Items / highlights / line groups carry `at`: the sentence index they appear on.
const fs = require('fs');
const path = require('path');
const REPO = path.resolve(__dirname, '../../..');
const LOGS = path.join(__dirname, 'build', 'logs');

const src = (rel, from, to) => {
  const all = fs.readFileSync(path.join(REPO, rel), 'utf8').split('\n');
  return { code: all.slice(from - 1, to).join('\n'), start: from, all };
};
// highlight lines [a,b] found by marker text, relative to file
const findLine = (all, marker, after = 0) => {
  const i = all.findIndex((l, k) => k >= after && l.includes(marker));
  if (i < 0) throw new Error('marker not found: ' + marker);
  return i + 1;
};

const strip = (s) => s.replace(/\x1b\[[0-9;]*m/g, '');
function testLines(log, filter) {
  const raw = strip(fs.readFileSync(path.join(LOGS, log), 'utf8')).split('\n');
  const out = [];
  for (const l of raw) {
    if (/^\s+Finished/.test(l)) out.push([l.trim(), 'd']);
    else if (/^\s+Running unittests/.test(l)) out.push(['Running unittests src/lib.rs', 'd']);
    else if (/^test .* \.\.\. ok$/.test(l) && (!filter || filter(l))) out.push([l, 'ok']);
    else if (/^test result: ok\. [1-9]/.test(l)) out.push([l, 'g']);
  }
  return out;
}

function siteLines() {
  return fs.readFileSync(path.join(LOGS, 'site.log'), 'utf8').split('\n')
    .filter((l) => /^ok \d|^# (tests|pass|fail)/.test(l))
    .map((l) => [l, /^ok/.test(l) ? 'ok' : /pass/.test(l) ? 'g' : '']);
}

const REPLAY18 = 'REPLAY · recorded 18 Sep 2026 · Surfnet fork of mainnet · slot 448083727 · docs/validation/2026-09-18-xsol-nav-surfnet.md';
const LIVE = 'RUN FOR THIS RECORDING · 25 Sep 2026 · real output, unedited';

const surfnetLines = [
  { at: 0, lines: [
    ['surfnet   http://127.0.0.1:8899', ''],
    ['mainnet   https://api.mainnet-beta.solana.com', ''],
    ['  deployed               svi-core 5yVpoQJC…ePECN, adapter FfK5xyE3…L3Mpr', 'd'],
    ['  snapshot               3 Hylo accounts re-cloned from mainnet', 'd'],
    ['  clock (pinned)         slot 448083727  epoch 1037', 'd'],
    ['  initialize_feed        ok   4JBPgCD2…', 'g'],
    ['  adapter initialize     ok   GfMaUsgd…', 'g'],
    ['  refresh_xsol_nav       ok   k4uKtVrZ…', 'g'],
    ['', ''],
  ]},
  { at: 1, lines: [
    ['  PUBLISHED QUOTE  2EhQfH26gQBxi8RteUVCXXA5MMnwuLZgFnMqQxLkseuc', 'a'],
    ['    quote_amount   $ 0.069715188', 'big'],
    ['    lower / upper  $ 0.069715188 .. $ 0.069809602   (13.54 bps)', ''],
    ['    base_amount      1000000 (1 whole xSOL)', ''],
    ['    observed_slot    448083727   valid_until 448084477', ''],
    ['    sequence         1   flags 0b10000 (BUY_ZONE)', ''],
    ['', ''],
  ]},
  { at: 2, lines: [
    ['  off-chain hylo-core:   $ 0.069715188', 'v'],
    ['  MATCH: on-chain program and off-chain reader agree to the last digit.', 'g'],
    ['', ''],
    ['test result: ok. 1 passed; 0 failed', 'g'],
  ]},
];

// ---------------------------------------------------------------- PITCH
const pyth = src('programs/svi-stock-adapter/programs/svi-stock-adapter/src/pyth.rs', 98, 199);
const P = pyth.all;
const L = (m) => findLine(P, m, 97);

const pitch = [
  { type: 'term', title: 'svi-hylo-adapter · surfnet end-to-end test', badge: REPLAY18, kind: 'replay',
    cmd: 'cargo test --test surfnet -- --nocapture',
    groups: [{ at: 0, lines: surfnetLines[0].lines.concat(surfnetLines[1].lines) }, { at: 2, lines: surfnetLines[2].lines }],
    s: ['This is what one xSOL is worth: six point nine seven cents.',
        'It is not quoted on any exchange, and no oracle publishes it.',
        'Until this account existed, the only way to know it was to write the maths yourself.'] },

  { type: 'claim', lines: [{ at: 0, t: 'Some assets have no price.' }, { at: 1, t: 'Not a stale one.', c: 'mut' }, { at: 2, t: 'None.', c: 'acc' }],
    s: ['Some assets have no price.', 'Not a stale one.', 'None.'] },

  { type: 'formula', kicker: 'WHERE THE NUMBER COMES FROM',
    s: ['xSOL is the leveraged tranche of Hylo\'s stablecoin.',
        'Its value is the collateral, times the SOL price, minus the hyUSD debt, divided by supply.',
        'Four numbers from Hylo\'s own books.',
        'Our program reads those accounts on-chain, does the arithmetic on-chain with Hylo\'s own published library, and writes the answer to an account.',
        'No server. No API key. Nothing to trust about us.'] },

  { type: 'image', src: 'repo:docs/diagrams/svi-architecture.png', kicker: 'ARCHITECTURE', pan: [[0.25, 0.0, 1.9], [0.5, 0.45, 1.6], [1, 0.62, 1.15]],
    s: ['Keepers are permissionless.',
        'Anyone can pay to crank a refresh, and nobody can change the number, because the core accepts a write only when the adapter program itself signed it.',
        'The quote account is the product: three hundred and twenty bytes, anyone can read it.'] },

  { type: 'card', kicker: 'MEASURED 18 SEP 2026 · scripts/check-pyth.py', title: 'The price feeds people assume are live',
    items: [
      { at: 1, k: 'Pyth equity accounts · mainnet', v: '6 – 34 days stale', c: 'red' },
      { at: 2, k: 'Pyth equity accounts · devnet', v: '78 days stale', c: 'red' },
      { at: 2, k: 'xStock accounts · devnet', v: '2 of 3 do not exist', c: 'red' },
      { at: 3, k: 'Who refused to use them?', v: 'nobody', c: 'amb' },
    ],
    s: ['Here is the problem this sits next to.',
        'On the eighteenth of September we measured the Pyth equity accounts people assume are live: six to thirty four days stale on mainnet.',
        'On devnet, seventy eight days, and two of three tokenized stock accounts do not exist at all.',
        'Nobody noticed, because nothing refuses. An oracle serves what it has, and leaves the judgement to you.'] },

  { type: 'code', file: 'programs/svi-stock-adapter/src/pyth.rs', code: pyth.code, start: pyth.start,
    hl: [
      { at: 1, a: L('// 1. Owner'), b: L('// 1. Owner') + 6 },
      { at: 2, a: L('// 4. Identity'), b: L('// 4. Identity') + 5 },
      { at: 3, a: L('// 3. Verification'), b: L('// 3. Verification') + 7 },
      { at: 4, a: L('// 5. Time sanity'), b: L('// 5. Time sanity') + 9 },
      { at: 5, a: L('// 6. Hard age limit'), b: L('return Err(error!(StockAdapterError::PythTooOld))') },
      { at: 6, a: L('// 8. Confidence'), b: L('// 8. Confidence') + 8 },
      { at: 7, a: L('// 8. Confidence'), b: L('// 8. Confidence') + 8 },
    ],
    s: ['Ours refuses.', 'Wrong owner.', 'Wrong asset.', 'Not fully verified.', 'Published in the future.', 'Too old.', 'Confidence too wide.',
        'Any one of those stops the write, and the quote goes stale visibly instead of going wrong quietly.'] },

  { type: 'claim', lines: [{ at: 0, t: 'Fail stale,' }, { at: 0, t: 'never fail wrong.', c: 'acc' }],
    s: ['Fail stale, never fail wrong.'] },

  { type: 'stocks',
    s: ['The same account format works for a different asset.',
        'A tokenized Apple share, against the real one.',
        'Pyth publishes both legs. Nobody publishes the gap between them, or says when the exchange is shut.',
        'SVI writes both quotes in one instruction, flagged market closed, reference stale, token feed stale, or deviation high.'] },

  { type: 'image', src: 'shot:site_full.png', kicker: 'THE PUBLIC PAGE · site/index.html', pan: [[0, 0, 1], [0.6, 0.06, 1], [1, 0.12, 1]], fitWidth: true,
    s: ['One format, any asset, written only by code that can prove its inputs.',
        'It is open source, anyone can crank it, and there is no token.',
        'The addresses are on the page. Read the same bytes from your own terminal.'] },

  { type: 'card', kicker: 'HONEST STATUS', title: 'Where this is today',
    items: [
      { at: 1, k: 'xSOL feed', v: 'ran on a mainnet fork — not mainnet', c: 'amb' },
      { at: 2, k: 'Stock feeds', v: 'blocked on a price source', c: 'red' },
      { at: 3, k: 'Consumers', v: 'none yet', c: 'amb' },
      { at: 3, k: 'Audit', v: 'none — do not collateralise against it', c: 'red' },
    ],
    s: ['To be plain about where this is.',
        'The xSOL feed ran on a mainnet fork, not mainnet.',
        'The stock feeds are blocked on a price source.',
        'No lending market reads it yet, and it is not audited, so nothing should be collateralised against it until it is.'] },

  { type: 'end', title: 'SVI', sub: 'the Solana Valuation Interface', url: 'github.com/princedotrs/solana-valuation-interface',
    s: ['But if you price collateral for a living, this is the number you should want to read at three in the morning.'] },
];

// ---------------------------------------------------------------- TECHNICAL
const core = src('programs/svi-core/programs/svi-core/src/state.rs', 131, 170);
const C = core.all;
const CL = (m) => findLine(C, m, 125);
const sst = src('programs/svi-stock-adapter/programs/svi-stock-adapter/src/state.rs', 41, 72);
const S = sst.all;
const SL = (m) => findLine(S, m, 38);

const tech = [
  { type: 'title', kicker: 'TECHNICAL WALKTHROUGH', title: 'SVI — the Solana Valuation Interface',
    bullets: [
      { at: 1, t: 'The xSOL quote, published and independently matched' },
      { at: 1, t: 'One 320-byte account for every asset' },
      { at: 1, t: 'The refusals' },
      { at: 1, t: 'The tests' },
      { at: 2, t: 'Test output: run for this recording. On-chain runs: replays, labelled with their date.', c: 'mut' },
    ],
    s: ['This is the technical walkthrough of SVI, the Solana Valuation Interface.',
        'Four things: the xSOL quote being published and independently matched, the account format, the refusals, and the tests.',
        'Everything shown is real code from the repository. Test output was run for this recording, and on-chain runs are replays of recorded runs, labelled with their date.'] },

  { type: 'term', title: 'svi-hylo-adapter · surfnet end-to-end test', badge: REPLAY18, kind: 'replay',
    cmd: 'cargo test --test surfnet -- --nocapture',
    groups: [{ at: 1, lines: surfnetLines[0].lines }, { at: 2, lines: surfnetLines[1].lines }, { at: 3, lines: surfnetLines[2].lines }],
    s: ['This is the end-to-end test, recorded on the eighteenth of September against a Surfnet fork of mainnet.',
        'One command deploys both programs, re-clones Hylo\'s three accounts from mainnet, pins the clock, refreshes, and reads the quote back.',
        'The program published six point nine seven cents per xSOL, with a band of thirteen and a half basis points, valid for seven hundred and fifty slots.',
        'The last line is an off-chain reader calling Hylo\'s library directly, with different decoding and scaling. It agrees to the ninth decimal.'] },

  { type: 'card', kicker: 'WHAT THE CLOCK PINNING IS, AND IS NOT', title: 'Pinned clock, unaltered bytes',
    items: [
      { at: 1, k: 'Hylo\'s oracle window', v: '≤ 10 s old · ≤ 25 slots away', c: '' },
      { at: 2, k: 'A local fork', v: 'fixed 400 ms slots, wall-clock time', c: 'amb' },
      { at: 3, k: 'The test', v: 'pins the clock to the snapshot\'s slot + timestamp', c: '' },
      { at: 4, k: 'Hylo bytes · Pyth bytes altered', v: 'none', c: 'grn' },
      { at: 5, k: 'Pyth update age at that slot', v: '1 second · 1 slot', c: 'grn' },
    ],
    s: ['The clock pinning deserves a straight explanation.',
        'Hylo will not price against an oracle update more than ten seconds old, or more than twenty five slots away.',
        'A local fork ticks at a fixed four hundred milliseconds while its clock follows real time, so after a few minutes both constraints cannot hold.',
        'So the test pins the clock to the slot and timestamp of the snapshot it just took.',
        'No Hylo byte is altered. No Pyth byte is altered.',
        'At that slot the Pyth update was one second and one slot old, well inside Hylo\'s own tolerance.'] },

  { type: 'code', file: 'programs/svi-core/src/state.rs', code: core.code, start: core.start,
    hl: [
      { at: 0, a: CL('pub struct Quote'), b: CL('pub struct Quote') },
      { at: 1, a: CL('// --- the value'), b: CL('// --- time and ordering') + 6 },
      { at: 2, a: CL('Fails the build'), b: CL('Fails the build') + 2 },
      { at: 3, a: CL('pub adapter_program'), b: CL('pub adapter_program') },
    ],
    s: ['Every asset uses one account shape.',
        'Value, a lower and upper bound, the slot it was observed at, the slot it stops being valid, a sequence number, and flags.',
        'Three hundred and twenty bytes. A compile time assertion fails the build if the layout drifts, because consumers read it by byte offset.',
        'A write is accepted only from the adapter PDA named in the descriptor. A compromised keeper cannot write a value. It can only pay for one to be computed.'] },

  { type: 'term', title: 'svi-core · unit tests', badge: LIVE, kind: 'live',
    cmd: 'cargo test --manifest-path programs/svi-core/programs/svi-core/Cargo.toml --lib',
    groups: [{ at: 0, lines: testLines('svi-core-lib.log') }],
    s: ['Here are the core\'s unit tests, run for this recording.',
        'Seven pass, including the layout test, and the flag range tests that stop two adapters from meaning different things by the same bit.'] },

  { type: 'code', file: 'programs/svi-stock-adapter/src/pyth.rs', code: pyth.code, start: pyth.start,
    hl: [
      { at: 0, a: L('pub fn load_verified'), b: L('pub fn load_verified') + 7 },
      { at: 1, a: L('// 1. Owner'), b: L('// 1. Owner') + 6 },
      { at: 2, a: L('// 2. Shape'), b: L('// 2. Shape') + 3 },
      { at: 3, a: L('// 3. Verification'), b: L('// 3. Verification') + 7 },
      { at: 4, a: L('// 4. Identity'), b: L('// 4. Identity') + 5 },
      { at: 5, a: L('// 4. Identity'), b: L('// 4. Identity') + 5 },
      { at: 6, a: L('// 5. Time sanity'), b: L('return Err(error!(StockAdapterError::PythTooOld))') },
      { at: 7, a: L('// 8. Confidence'), b: L('// 8. Confidence') + 8 },
      { at: 8, a: L('// 8. Confidence'), b: L('// 8. Confidence') + 8 },
    ],
    s: ['Now the stock adapter\'s price loader. Most of this program is refusals, and that is deliberate.',
        'One: the account must be owned by the Pyth receiver.',
        'Two: it must deserialize.',
        'Three: its verification level must meet the minimum this symbol demands.',
        'Four, the important one: the feed id inside the account must equal the one in the config.',
        'A keeper chooses which accounts go into a transaction. Without this check, anyone could pass a real, fully verified Pyth price for a cheaper asset, and have it published as Apple\'s.',
        'Then the publish time must be positive and not from the future, and the price no older than the hard limit.',
        'And the confidence band can be no wider than five percent.',
        'Any failure reverts the transaction. Nothing is published, and the previous quote simply expires.'] },

  { type: 'code', file: 'programs/svi-stock-adapter/src/state.rs', code: sst.code, start: sst.start,
    hl: [
      { at: 0, a: SL('// ---- staleness policy'), b: SL('// ---- staleness policy') + 6 },
      { at: 2, a: SL('// ---- staleness policy') + 2, b: SL('// ---- staleness policy') + 6 },
      { at: 3, a: SL('pub market_closed_secs'), b: SL('pub reference_max_age_secs') },
      { at: 4, a: SL('pub max_conf_bps') - 4, b: SL('pub max_conf_bps') },
    ],
    s: ['The design decision the stock feed turns on is two windows per feed.',
        'Too old to use, and old enough to mention, are different questions.',
        'An equity price is supposed to be hours old overnight. Refusing then would take the feed dark exactly when a consumer most needs to be told the market is shut.',
        'So fifteen minutes raises market closed. Four days adds reference stale. Eight days refuses outright.',
        'Every threshold lives in the symbol\'s on-chain config, so anyone can read the rules that produced a quote.'] },

  { type: 'term', title: 'svi-stock-adapter · unit tests', badge: LIVE, kind: 'live',
    cmd: 'cargo test --manifest-path programs/svi-stock-adapter/programs/svi-stock-adapter/Cargo.toml',
    groups: [{ at: 0, lines: testLines('svi-stock-adapter.log') }],
    s: ['The adapter\'s tests pin those rules down.',
        'Overnight sets market closed only. A long weekend does not set reference stale. A deviation exactly at tolerance is not flagged, and one basis point over is.',
        'Twenty six pass.'] },

  { type: 'term', title: 'svi-keeper · unit tests', badge: LIVE, kind: 'live',
    cmd: 'cargo test --manifest-path tools/svi-keeper/Cargo.toml',
    groups: [{ at: 0, lines: testLines('svi-keeper.log') }],
    s: ['The keeper is the permissionless crank. It holds no privileged key.',
        'Its tests check that refresh instructions match the program\'s account order, and that a pair file naming the same account twice is rejected.',
        'They also check that on-chain error codes decode into something an operator can act on. The staleness refusal maps to six thousand and six.',
        'Thirty one pass.'] },

  { type: 'card', kicker: 'WHY THE STOCK FEEDS ARE NOT LIVE · measured 18 Sep 2026', title: 'scripts/check-pyth.py',
    items: [
      { at: 1, k: 'Pyth sponsored equity accounts · mainnet', v: '6 – 34 days stale', c: 'red' },
      { at: 1, k: 'Pyth sponsored equity accounts · devnet', v: '78 days stale', c: 'red' },
      { at: 2, k: 'xStock price accounts · devnet', v: '2 of 3 missing', c: 'red' },
      { at: 2, k: 'Hermes without a key', v: 'HTTP 401', c: 'red' },
      { at: 3, k: 'Verdicts', v: 'usable now · bring your own price · do not proceed', c: '' },
    ],
    s: ['So why are the stock feeds not live? We measured it.',
        'On the eighteenth of September, Pyth\'s sponsored equity accounts were six to thirty four days stale on mainnet, and seventy eight on devnet.',
        'Two of three xStock accounts were missing on devnet, and Hermes began answering four oh one without a key.',
        'The script gives three verdicts, because they need three different responses: usable now, needs a price of your own, or do not proceed.'] },

  { type: 'term', title: 'site · decoder and empty-state tests', badge: LIVE, kind: 'live',
    cmd: 'node --test site/test/*.mjs',
    groups: [{ at: 1, lines: siteLines() }],
    s: ['The dashboard reads the quote accounts straight from the browser over public RPC, and decodes the layout itself. There is no backend.',
        'When there is no current quote, it withholds the number rather than showing it next to a warning. Its fifteen decoder and empty state tests pass.'] },

  { type: 'card', kicker: 'WHAT IS NOT DONE', title: 'Said plainly',
    items: [
      { at: 0, k: 'Consumers', v: 'no lending market reads the feed yet', c: 'amb' },
      { at: 1, k: 'Audit', v: 'none — do not collateralise against it', c: 'red' },
      { at: 2, k: 'Stock feeds', v: 'blocked: no price source, not shipped', c: 'red' },
      { at: 3, k: 'xSOL feed', v: 'works today, on a mainnet fork — not mainnet', c: 'amb' },
      { at: 4, k: 'methodology_hash', v: 'zero until the spec is frozen', c: 'amb' },
    ],
    s: ['What is not done, said plainly. No consumer reads this feed yet.',
        'There is no audit, and nothing should be collateralised against it until there is.',
        'The stock feeds have no price source: the sponsored accounts stopped updating and Hermes needs a key. They are blocked, not shipped.',
        'The xSOL feed needs none of that, which is why it works today. It runs on a mainnet fork, not on mainnet.',
        'The methodology hash is still zero, because the spec is not frozen.'] },

  { type: 'end', title: 'SVI', sub: 'Fail stale, never fail wrong.', url: 'github.com/princedotrs/solana-valuation-interface',
    s: ['All of it is written in the repository, in the same words.'] },
];

module.exports = { pitch, tech };

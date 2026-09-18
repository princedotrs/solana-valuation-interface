/**
 * The page's claim is that the adapter refuses rather than publishes a value
 * it cannot stand behind. A page that renders an expired value in the same
 * type as a live one, with a small tag beside it, breaks that claim for
 * everyone who reads the number and not the tag -- which is most people, and
 * all of them when glancing.
 *
 * So: an expired quote must WITHHOLD its value, not decorate it.
 *
 * These run the page's own `card` and `agoFromSlots`, extracted from the
 * shipped HTML, for the same reason decode.test.mjs does: a test against a
 * copy of the logic passes while the page does something else.
 *
 *     node --test site/test/*.mjs
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const page = readFileSync(new URL('../index.html', import.meta.url), 'utf8');

/** Pull one `function name(...) {...}` out of the page, balancing braces. */
function extract(name) {
  const start = page.indexOf(`function ${name}(`);
  assert.notEqual(start, -1, `the page no longer defines ${name}`);
  let depth = 0, i = page.indexOf('{', start);
  const from = i;
  for (; i < page.length; i++) {
    if (page[i] === '{') depth++;
    else if (page[i] === '}' && --depth === 0) break;
  }
  return `function ${name}(${page.slice(start + `function ${name}(`.length, from).trimEnd()}
          ${page.slice(from, i + 1)}`;
}

// The helpers `card` closes over, kept deliberately simple: the assertions
// below are about which values appear, never about their formatting.
const harness = `
  const usd = (v) => '$' + (Number(v) / 1e9).toFixed(4);
  const premium = (m, f) => ((Number(m) - Number(f)) / Number(f)) * 100;
  const short = (a) => String(a).slice(0, 4);
  const flagChips = () => '<span class="chip ok">clear</span>';
  const EXPLORER_Q = '?cluster=devnet';
  ${extract('agoFromSlots')}
  ${extract('card')}
  return { card, agoFromSlots };
`;
const { card, agoFromSlots } = new Function(harness)();

const quote = (over = {}) => Object.assign({
  value: 365_230_000_000n, lower: 360_000_000_000n, upper: 370_000_000_000n,
  observed_slot: 1000n, valid_until_slot: 1750n, sequence: 7n,
  status_flags: 0, addr: 'FxQ2AM2MkZ2ZNMUhJJwbVnV5DKDUHZC7KfYXFJABiegQ',
}, over);

test('a live quote shows its value and its premium', () => {
  const html = card('TSLA', quote(), quote({ value: 365_275_000_000n }), 1200n);
  assert.match(html, /\$365\.2300/, 'the fair value should be shown');
  assert.doesNotMatch(html, /withheld/, 'nothing is withheld while current');
  assert.doesNotMatch(html, /not being cranked/);
  assert.match(html, /[+-]\d+\.\d\d%/, 'a premium should be computed');
});

test('an expired quote withholds its value rather than labelling it', () => {
  // slot is past valid_until_slot on both legs.
  const html = card('TSLA', quote(), quote(), 9999n);
  assert.match(html, /withheld/, 'the value must be withheld');
  assert.doesNotMatch(html, /\$365\.2300/,
    'an expired value must not be rendered as a number anywhere on the card');
  assert.doesNotMatch(html, /\$360\.0000/, 'bounds belong to a withheld value');
});

test('an expired card computes no premium', () => {
  const html = card('TSLA', quote(), quote({ value: 400_000_000_000n }), 9999n);
  assert.doesNotMatch(html, /[+-]\d+\.\d\d%/,
    'a premium is arithmetic on a value we just disowned');
  assert.match(html, /no quote/, 'the chip should say there is no quote');
});

test('an expired card says why, and how far past', () => {
  const html = card('TSLA', quote(), quote(), 1750n + 9000n);
  assert.match(html, /not being cranked/);
  assert.match(html, /9000 slots past its validity/);
  assert.match(html, /class="dash-card expired"/, 'the card should be marked expired');
});

test('one expired leg poisons the pair', () => {
  // Fair is current, market is long past. A premium across the two would
  // compare different moments, which is worse than showing nothing.
  const html = card('TSLA', quote({ valid_until_slot: 99999n }), quote(), 5000n);
  assert.doesNotMatch(html, /[+-]\d+\.\d\d%/);
  assert.match(html, /withheld/);
});

test('a missing leg is reported, not rendered as zero', () => {
  const html = card('TSLA', null, quote(), 1200n);
  assert.match(html, /has not been published yet/);
  assert.doesNotMatch(html, /\$0\.0000/);
});

test('slot ages read as time, and stay vague about it', () => {
  assert.match(agoFromSlots(10n), /about a minute/);
  assert.match(agoFromSlots(1500n), /about 10 minutes/);      // 600s
  assert.match(agoFromSlots(90000n), /about 10 hours/);       // 36000s
  assert.match(agoFromSlots(2160000n), /about 10 days/);      // 864000s
});

test('an expired card does not report its flags as clear', () => {
  // `clear` is green and means "nothing to report". Next to a quote we have
  // just refused to show, it reads as "all is well", which is the opposite of
  // what the card says two lines above.
  const html = card('TSLA', quote(), quote(), 9999n);
  assert.doesNotMatch(html, /chip ok/, 'no green chip on an expired card');
  assert.doesNotMatch(html, />clear</, 'must not claim the flags are clear');
  assert.match(html, /as of the last quote/, 'the flags must be dated');
});

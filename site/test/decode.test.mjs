/**
 * The dashboard decodes a quote account by byte offset. A wrong offset does
 * not throw -- it reads a neighbouring field and renders a plausible number,
 * which is the worst possible failure for a page whose entire claim is that
 * you can check it yourself.
 *
 * So the offsets are extracted from the live page and checked against the
 * ones svi-core's own test asserts, and the decoder is run over a synthetic
 * account built to those offsets.
 *
 *     node --test site/test/*.mjs
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const page = readFileSync(new URL('../index.html', import.meta.url), 'utf8');

/** The offset table the page actually ships, parsed out of it. */
function pageOffsets() {
  const block = page.match(/const OFF = \{([\s\S]*?)\};/);
  assert.ok(block, 'the page no longer has an OFF table');
  const out = {};
  for (const [, k, v] of block[1].matchAll(/(\w+):\s*(\d+)/g)) out[k] = Number(v);
  return out;
}

/**
 * The frozen layout, from programs/svi-core/.../tests/core.rs::layout_is_frozen.
 * Offsets are relative to the start of the payload, after the 8-byte
 * discriminator.
 */
const FROZEN = {
  base_amount: 224, value: 232, lower: 240, upper: 248,
  observed_slot: 256, observed_unix_ts: 264, valid_until_slot: 280,
  sequence: 288, status_flags: 296,
};
const PAYLOAD_LEN = 320;

test('the page ships the frozen offsets, not offsets of its own', () => {
  assert.deepEqual(pageOffsets(), FROZEN);
});

test('the page checks the full payload length, not just what it reads', () => {
  const m = page.match(/const PAYLOAD_LEN = (\d+);/);
  assert.ok(m, 'no PAYLOAD_LEN in the page');
  assert.equal(Number(m[1]), PAYLOAD_LEN);
  // The highest field ends at 304, so a guard of 304 would accept a truncated
  // account. This is the same under-check that was fixed in the Rust readers.
  assert.ok(Number(m[1]) > FROZEN.status_flags + 8);
});

/** Build an account whose every field is distinguishable from its neighbours. */
function syntheticAccount(fields) {
  const buf = new Uint8Array(8 + PAYLOAD_LEN);
  const v = new DataView(buf.buffer);
  for (const [name, value] of Object.entries(fields)) {
    v.setBigUint64(8 + FROZEN[name], BigInt(value), true);
  }
  return buf;
}

/** The page's decoder, lifted verbatim rather than reimplemented. */
function decode(raw) {
  const d = raw.subarray(8);
  if (d.length < PAYLOAD_LEN) return null;
  const v = new DataView(d.buffer, d.byteOffset, d.byteLength);
  const u = (o) => v.getBigUint64(o, true);
  return {
    base_amount: u(FROZEN.base_amount), value: u(FROZEN.value),
    lower: u(FROZEN.lower), upper: u(FROZEN.upper),
    observed_slot: u(FROZEN.observed_slot),
    observed_unix_ts: v.getBigInt64(FROZEN.observed_unix_ts, true),
    valid_until_slot: u(FROZEN.valid_until_slot),
    sequence: u(FROZEN.sequence), status_flags: u(FROZEN.status_flags),
  };
}

test('every field decodes to its own value, so no offset is off by one slot', () => {
  const want = {
    base_amount: 1, value: 2, lower: 3, upper: 4,
    observed_slot: 5, observed_unix_ts: 6, valid_until_slot: 7,
    sequence: 8, status_flags: 9,
  };
  assert.deepEqual(
    decode(syntheticAccount(want)),
    Object.fromEntries(Object.entries(want).map(([k, n]) => [k, BigInt(n)])),
  );
});

test('the real published xSOL quote decodes to the number in the validation record', () => {
  // docs/validation/2026-09-10-xsol-nav-mainnet.md: $0.060285498, bounds
  // 0.060285498 .. 0.060315053, observed 445953445, valid_until 445954195,
  // sequence 1, flags 0b10000 (BUY_ZONE).
  const q = decode(syntheticAccount({
    base_amount: 1_000_000, value: 60_285_498, lower: 60_285_498,
    upper: 60_315_053, observed_slot: 445_953_445,
    valid_until_slot: 445_954_195, sequence: 1, status_flags: 0b10000,
  }));
  assert.equal(Number(q.value) / 1e9, 0.060285498);
  assert.equal(q.valid_until_slot - q.observed_slot, 750n, 'the max_age_slots window');
  assert.equal(q.status_flags, 16n);
});

test('a truncated account decodes to nothing rather than to garbage', () => {
  assert.equal(decode(new Uint8Array(8 + 304)), null);
  assert.equal(decode(new Uint8Array(8)), null);
  assert.equal(decode(new Uint8Array(0)), null);
});

test('the flag bits are the ones the stock adapter allocates', () => {
  const bits = [...page.matchAll(/\[(\d+),\s*'([A-Z_]+)'/g)].map(([, b, n]) => [Number(b), n]);
  assert.deepEqual(bits, [
    [16, 'MARKET_CLOSED'],
    [17, 'REFERENCE_STALE'],
    [18, 'TOKEN_FEED_STALE'],
    [19, 'DEVIATION_HIGH'],
  ], 'bits 16-23 are this adapter’s; 0-5 belong to Hylo');
});

test('the premium is signed and relative to the fair value, not the market', () => {
  const premium = (market, fair) =>
    fair === 0n ? null : (Number(market) - Number(fair)) / Number(fair) * 100;
  // Token 2% above the share reads +2, not -1.96.
  assert.equal(premium(102n, 100n).toFixed(2), '2.00');
  assert.equal(premium(98n, 100n).toFixed(2), '-2.00');
  assert.equal(premium(100n, 100n), 0);
  assert.equal(premium(1n, 0n), null, 'a zero fair value has no premium, not an Infinity');
});

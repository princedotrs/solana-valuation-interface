#!/usr/bin/env python3
"""Ask a cluster whether the Pyth price accounts this deployment needs exist.

This answers the one question that decides how the whole deployment runs, and
answers it before you spend SOL rather than after a transaction fails:

    Does Pyth sponsor a price account for this symbol on this cluster?

    yes -> the keeper reads it. Full verification. Nothing to post.
    no  -> the keeper must bring its own price with `--post-updates`, which
           yields Partial verification, which means that symbol's
           `min_verification_level` has to be 0 at initialize time.

Getting this wrong is not subtle but it is expensive: you deploy, configure
every symbol to demand Full verification, and then every single refresh aborts
with PythNotFullyVerified (6002) and the fix is a config you can only set when
the symbol is created.

    python3 scripts/check-pyth.py                          # devnet, from config/
    python3 scripts/check-pyth.py --rpc https://api.mainnet-beta.solana.com
    python3 scripts/check-pyth.py --deployments deployments.json
    python3 scripts/check-pyth.py --self-test              # no network

Reads `config/pyth-feeds.json` by default, or `deployments.json` with
--deployments, so it works both before and after the addresses are planned.
"""

from __future__ import annotations

import argparse
import base64
import json
import struct
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from netutil import diagnose, tls_context  # noqa: E402
from solana_pda import pda  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
FEEDS_IN = ROOT / "config" / "pyth-feeds.json"

# From `pyth_solana_receiver_sdk`: the receiver owns price accounts, the push
# oracle is the PDA namespace sponsored feeds live in.
PYTH_RECEIVER = "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ"
PYTH_PUSH_ORACLE = "pythWSnswVUd12oZpeFP8e9CVaEqJg25g1Vtc2biRsT"

DEFAULT_RPC = "https://api.devnet.solana.com"

# Thresholds mirrored from scripts/plan-deployment.py, so this reports the same
# verdict the adapter will reach on-chain.
MARKET_CLOSED_SECS = 15 * 60
REFERENCE_MAX_AGE_SECS = 8 * 24 * 3600
TOKEN_MAX_AGE_SECS = 30 * 60
MAX_CONF_BPS = 500


class PriceUpdate:
    """A decoded `PriceUpdateV2`.

    Layout, from the receiver program's own struct:

        8   discriminator
        32  write_authority
        1   verification_level tag  (0 = Partial, 1 = Full)
        1   num_signatures          (only when the tag is Partial)
        84  price_message
        8   posted_slot

    The variable-width enum in the middle is the whole reason this is written
    out by hand: reading the price at a fixed offset works for Full updates and
    silently reads one byte off for Partial ones.
    """

    __slots__ = ("full", "num_signatures", "feed_id", "price", "conf",
                 "exponent", "publish_time", "posted_slot")

    def __init__(self, raw: bytes):
        if len(raw) < 8 + 32 + 1:
            raise ValueError(f"account is {len(raw)} bytes, too short for a price update")
        tag = raw[40]
        if tag == 0:
            self.full = False
            self.num_signatures = raw[41]
            off = 42
        elif tag == 1:
            self.full = True
            self.num_signatures = None
            off = 41
        else:
            raise ValueError(f"verification level tag is {tag}, expected 0 or 1")

        end = off + 84
        if len(raw) < end + 8:
            raise ValueError(f"account is {len(raw)} bytes, too short for its price message")
        msg = raw[off:end]
        self.feed_id = msg[0:32].hex()
        # i64 price, u64 conf, i32 exponent, i64 publish_time — little-endian.
        self.price, self.conf, self.exponent, self.publish_time = struct.unpack_from("<qqiq", msg, 32)
        self.conf &= (1 << 64) - 1
        self.posted_slot = struct.unpack_from("<Q", raw, end)[0]

    def usd(self) -> float:
        return self.price * (10 ** self.exponent)

    def conf_bps(self) -> float:
        return 0.0 if self.price == 0 else abs(self.conf / self.price) * 10_000

    def level(self) -> str:
        return "Full" if self.full else f"Partial/{self.num_signatures}"


def rpc(url: str, method: str, params) -> dict:
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url, data=body, headers={"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=30, context=tls_context()) as r:
        out = json.loads(r.read().decode())
    if "error" in out:
        raise RuntimeError(f"{method}: {out['error'].get('message', out['error'])}")
    return out["result"]


def fetch_account(url: str, address: str) -> bytes | None:
    res = rpc(url, "getAccountInfo", [address, {"encoding": "base64", "commitment": "confirmed"}])
    value = res.get("value")
    if value is None:
        return None
    return base64.b64decode(value["data"][0]), value["owner"]


def describe(url: str, label: str, address: str, want_feed_id: str,
             max_age: int, now: int) -> tuple[bool, list[str]]:
    """Report on one feed. Returns (usable, notes)."""
    notes: list[str] = []
    try:
        got = fetch_account(url, address)
    except (urllib.error.URLError, TimeoutError, RuntimeError) as exc:
        notes = [f"{label}: could not reach the RPC: {exc}"]
        hint = diagnose(exc)
        if hint:
            notes += ["    " + ln for ln in hint.splitlines()]
        return False, notes

    if got is None:
        return False, [
            f"{label}: NO ACCOUNT at {address}",
            f"    Pyth does not sponsor this feed on this cluster.",
            f"    The keeper must post its own updates (--post-updates), and this",
            f"    symbol needs min_verification_level = 0.",
        ]

    raw, owner = got
    if owner != PYTH_RECEIVER:
        return False, [
            f"{label}: account exists but is owned by {owner},",
            f"    not the Pyth receiver ({PYTH_RECEIVER}).",
            f"    The adapter would abort with PythWrongOwner (6000).",
        ]

    try:
        u = PriceUpdate(raw)
    except ValueError as exc:
        return False, [f"{label}: account does not decode as a price update: {exc}"]

    age = now - u.publish_time
    ok = True

    notes.append(
        f"{label}: ${u.usd():,.4f}  +/-{u.conf_bps():.1f}bps  "
        f"{u.level()}  {age}s old  (slot {u.posted_slot})"
    )

    if u.feed_id != want_feed_id:
        ok = False
        notes.append(f"    WRONG FEED: account holds {u.feed_id[:16]}…, expected {want_feed_id[:16]}…")
        notes.append(f"    The adapter would abort with PythFeedMismatch (6003).")
    if not u.full:
        notes.append(f"    Partial verification: this symbol needs min_verification_level = 0.")
    if age > max_age:
        ok = False
        notes.append(f"    TOO OLD: {age}s exceeds the {max_age}s hard limit — PythTooOld (6006).")
    if u.conf_bps() > MAX_CONF_BPS:
        ok = False
        notes.append(f"    CONFIDENCE TOO WIDE: {u.conf_bps():.0f}bps over {MAX_CONF_BPS} — PythConfidenceTooWide (6008).")
    return ok, notes


def load_symbols(path: Path) -> list[dict]:
    doc = json.loads(path.read_text())
    if "symbols" in doc:                      # config/pyth-feeds.json
        return [{"symbol": s["symbol"],
                 "equity": s["equity"]["feed_id"],
                 "token": s["token"]["feed_id"],
                 "equity_addr": None, "token_addr": None} for s in doc["symbols"]]
    if "stocks" in doc:                        # deployments.json
        return [{"symbol": sym,
                 "equity": v["feed_ids"]["equity"],
                 "token": v["feed_ids"]["token"],
                 "equity_addr": v.get("equity_price"),
                 "token_addr": v.get("token_price")} for sym, v in doc["stocks"].items()]
    raise ValueError(f"{path} has neither a 'symbols' nor a 'stocks' section")


def self_test() -> int:
    """Build price accounts byte by byte and check both enum widths decode."""
    def build(full: bool, feed_id: bytes, price: int, conf: int, expo: int,
              publish: int, slot: int, num_sigs: int = 5) -> bytes:
        raw = b"\x00" * 8 + b"\x11" * 32
        raw += b"\x01" if full else bytes([0, num_sigs])
        raw += feed_id + struct.pack("<qqiq", price, conf, expo, publish)
        raw += b"\x00" * 24                     # prev_publish, ema_price, ema_conf
        raw += struct.pack("<Q", slot)
        return raw

    fid = bytes(range(32))
    # Full: $250.00 at expo -8, conf 5 bps.
    u = PriceUpdate(build(True, fid, 25_000_000_000, 12_500_000, -8, 1_700_000_000, 999))
    assert u.full and u.num_signatures is None
    assert u.feed_id == fid.hex()
    assert abs(u.usd() - 250.0) < 1e-9, u.usd()
    assert abs(u.conf_bps() - 5.0) < 1e-6, u.conf_bps()
    assert u.publish_time == 1_700_000_000 and u.posted_slot == 999
    assert u.level() == "Full"

    # Partial: the extra byte must shift every field. A decoder that ignores it
    # reads a garbage feed id, which is exactly the bug this guards.
    p = PriceUpdate(build(False, fid, 25_000_000_000, 12_500_000, -8, 1_700_000_000, 999))
    assert not p.full and p.num_signatures == 5
    assert p.feed_id == u.feed_id, "Partial and Full must decode to the same feed"
    assert p.usd() == u.usd() and p.posted_slot == u.posted_slot
    assert p.level() == "Partial/5"

    # A wrong tag is an error, not a silent misread.
    bad = bytearray(build(True, fid, 1, 0, 0, 1, 1)); bad[40] = 7
    try:
        PriceUpdate(bytes(bad))
    except ValueError:
        pass
    else:
        raise AssertionError("an unknown verification tag must be rejected")

    for short in (b"", b"\x00" * 40, b"\x00" * 100):
        try:
            PriceUpdate(short)
        except ValueError:
            pass
        else:
            raise AssertionError(f"{len(short)} bytes should have been rejected")

    # A negative price is a real Pyth value and must not wrap.
    n = PriceUpdate(build(True, fid, -5_000, 0, -2, 1, 1))
    assert n.usd() < 0, n.usd()

    print("check-pyth self-test ok: both enum widths, wrong tag, short input, sign")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--rpc", default=DEFAULT_RPC)
    ap.add_argument("--feeds", type=Path, default=FEEDS_IN)
    ap.add_argument("--deployments", type=Path)
    ap.add_argument("--shard", type=int, default=0)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    path = args.deployments or args.feeds
    if not path.exists():
        print(f"{path} does not exist. Run scripts/fetch-feed-ids.py first.", file=sys.stderr)
        return 1

    try:
        symbols = load_symbols(path)
    except (ValueError, KeyError) as exc:
        print(f"{path}: {exc}", file=sys.stderr)
        return 1

    print(f"cluster   {args.rpc}")
    print(f"source    {path}")
    try:
        now = int(rpc(args.rpc, "getBlockTime", [rpc(args.rpc, "getSlot", [])]))
        print(f"chain now {datetime.fromtimestamp(now, timezone.utc).isoformat()}\n")
    except Exception as exc:
        print(f"could not read the chain clock ({exc}); falling back to local time\n")
        now = int(datetime.now(timezone.utc).timestamp())

    sponsored, needs_posting = [], []
    for s in symbols:
        print(f"{s['symbol']}")
        eq_addr = s["equity_addr"] or pda(PYTH_PUSH_ORACLE,
                                          [args.shard.to_bytes(2, "little"), bytes.fromhex(s["equity"])])
        tk_addr = s["token_addr"] or pda(PYTH_PUSH_ORACLE,
                                         [args.shard.to_bytes(2, "little"), bytes.fromhex(s["token"])])
        eq_ok, eq_notes = describe(args.rpc, "  equity", eq_addr, s["equity"], REFERENCE_MAX_AGE_SECS, now)
        tk_ok, tk_notes = describe(args.rpc, "  token ", tk_addr, s["token"], TOKEN_MAX_AGE_SECS, now)
        for n in eq_notes + tk_notes:
            print(n)
        print()
        (sponsored if (eq_ok and tk_ok) else needs_posting).append(s["symbol"])

    print("-" * 64)
    if sponsored:
        print(f"READ DIRECTLY ({len(sponsored)}): {', '.join(sponsored)}")
        print("  Both feeds are live and usable. Crank with:")
        print("    svi-keeper crank --feed stock:<SYM>")
        print("  Keep min_verification_level = 1 unless a note above says otherwise.")
    if needs_posting:
        print(f"NEEDS --post-updates ({len(needs_posting)}): {', '.join(needs_posting)}")
        print("  Set min_verification_level = 0 for these AT INITIALIZE TIME, then:")
        print("    svi-keeper crank --feed stock:<SYM> --post-updates")
    return 0 if sponsored else 2


if __name__ == "__main__":
    sys.exit(main())

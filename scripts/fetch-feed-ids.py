#!/usr/bin/env python3
"""Fetch Pyth feed ids for a set of tokenized stocks, and write them to config.

Why this exists rather than a table of constants in the source: a feed id is
32 bytes that decide which price a program will accept. Getting one wrong does
not fail loudly — it publishes a real, fully-verified price for the wrong
asset. So the ids come from Pyth's own published list at a recorded moment,
and the file this writes carries the URL and the timestamp it came from.

    python3 scripts/fetch-feed-ids.py                    # AAPL, TSLA, NVDA
    python3 scripts/fetch-feed-ids.py AAPL TSLA NVDA MSFT
    python3 scripts/fetch-feed-ids.py --self-test        # no network

Each symbol needs BOTH legs to be usable:

    equity   Equity.US.<SYM>/USD     the real share
    token    Crypto.<SYM>X/USD       the tokenized version

A symbol missing either leg is reported and left out of the config rather than
half-written, because a pair is the unit this adapter publishes. If too few
symbols survive, pick different ones — the script prints what it found.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from netutil import diagnose, tls_context  # noqa: E402



HERMES = "https://hermes.pyth.network"
FEEDS_PATH = "/v2/price_feeds"
DEFAULT_SYMBOLS = ["AAPL", "TSLA", "NVDA"]
OUT = Path(__file__).resolve().parent.parent / "config" / "pyth-feeds.json"

FEED_ID_RE = re.compile(r"^[0-9a-f]{64}$")


def equity_symbol(sym: str) -> str:
    return f"Equity.US.{sym}/USD"


def token_symbol(sym: str) -> str:
    """xStock tickers append an X: AAPL -> AAPLx, quoted by Pyth as AAPLX."""
    return f"Crypto.{sym}X/USD"


def normalise_id(raw: str) -> str:
    """Pyth writes ids with and without a 0x prefix depending on the surface."""
    return raw[2:].lower() if raw.lower().startswith("0x") else raw.lower()


def index_by_symbol(payload: object) -> dict[str, str]:
    """Map `attributes.symbol` -> feed id from a Hermes /v2/price_feeds body.

    Tolerant on purpose: Hermes has moved fields between releases, and a silent
    mismatch here would produce an empty config that looks like "no such feed"
    rather than "the response shape changed". Anything unparseable is skipped
    and counted by the caller.
    """
    out: dict[str, str] = {}
    if not isinstance(payload, list):
        return out
    for entry in payload:
        if not isinstance(entry, dict):
            continue
        feed_id = entry.get("id")
        attrs = entry.get("attributes")
        symbol = attrs.get("symbol") if isinstance(attrs, dict) else None
        # Older shapes put the symbol at the top level.
        if symbol is None:
            symbol = entry.get("symbol")
        if isinstance(feed_id, str) and isinstance(symbol, str):
            out[symbol] = normalise_id(feed_id)
    return out


# Set by --from-file: a Hermes response captured elsewhere. The filtering and
# pairing below are unchanged, so a file and a live fetch reach the same result.
FROM_FILE: object | None = None


def fetch(query: str, asset_type: str | None) -> tuple[object, str]:
    params = {"query": query}
    if asset_type:
        params["asset_type"] = asset_type
    url = f"{HERMES}{FEEDS_PATH}?{urllib.parse.urlencode(params)}"
    if FROM_FILE is not None:
        return FROM_FILE, f"{url}  (served from a local file)"
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=30, context=tls_context()) as resp:
        return json.loads(resp.read().decode()), url


def resolve(sym: str) -> tuple[dict | None, list[str]]:
    """Return the pair for one symbol, plus any problems found."""
    problems: list[str] = []
    want_equity, want_token = equity_symbol(sym), token_symbol(sym)

    try:
        equity_body, equity_url = fetch(sym, "equity")
        token_body, token_url = fetch(f"{sym}X", "crypto")
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as exc:
        hint = diagnose(exc)
        if hint:
            print(f"\n{hint}\n", file=sys.stderr)
        return None, [f"{sym}: Hermes request failed: {exc}"]

    equity_map = index_by_symbol(equity_body)
    token_map = index_by_symbol(token_body)

    if not equity_map and not token_map:
        problems.append(
            f"{sym}: Hermes returned nothing parseable. First bytes of the "
            f"equity response: {str(equity_body)[:200]}"
        )

    equity_id = equity_map.get(want_equity)
    token_id = token_map.get(want_token)

    if equity_id is None:
        near = [s for s in equity_map if sym in s][:5]
        problems.append(f"{sym}: no {want_equity}. Close matches: {near or 'none'}")
    if token_id is None:
        near = [s for s in token_map if sym in s][:5]
        problems.append(f"{sym}: no {want_token}. Close matches: {near or 'none'}")

    for label, fid in (("equity", equity_id), ("token", token_id)):
        if fid is not None and not FEED_ID_RE.match(fid):
            problems.append(f"{sym}: {label} id is not 32 hex bytes: {fid!r}")
            return None, problems

    if equity_id is None or token_id is None:
        return None, problems
    if equity_id == token_id:
        problems.append(f"{sym}: both legs resolved to the same feed id")
        return None, problems

    return {
        "symbol": sym,
        "equity": {"pyth_symbol": want_equity, "feed_id": equity_id, "source": equity_url},
        "token": {"pyth_symbol": want_token, "feed_id": token_id, "source": token_url},
    }, problems


def self_test() -> int:
    """Exercise the parsing and pairing logic with no network."""
    sample = [
        {
            "id": "0xAAAA" + "b" * 60,
            "attributes": {"symbol": "Equity.US.AAPL/USD", "asset_type": "Equity"},
        },
        {"id": "c" * 64, "attributes": {"symbol": "Crypto.AAPLX/USD"}},
        {"id": "not-an-id", "attributes": {}},
        "garbage",
    ]
    idx = index_by_symbol(sample)
    assert idx["Equity.US.AAPL/USD"] == "aaaa" + "b" * 60, idx
    assert idx["Crypto.AAPLX/USD"] == "c" * 64, idx
    assert len(idx) == 2, f"unparseable entries should be skipped, got {idx}"

    assert index_by_symbol({"not": "a list"}) == {}
    assert index_by_symbol([]) == {}
    assert normalise_id("0xABC") == "abc"
    assert normalise_id("ABC") == "abc"
    assert equity_symbol("TSLA") == "Equity.US.TSLA/USD"
    assert token_symbol("TSLA") == "Crypto.TSLAX/USD"
    assert FEED_ID_RE.match("e" * 64)
    assert not FEED_ID_RE.match("E" * 64), "ids are normalised to lowercase first"
    assert not FEED_ID_RE.match("e" * 63)

    print("self-test ok: parsing, normalisation and symbol construction")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("symbols", nargs="*", default=None)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--out", type=Path, default=OUT)
    ap.add_argument("--from-file", type=Path, metavar="JSON",
                    help="Read a saved Hermes /v2/price_feeds response instead of "
                         "fetching. Use when this machine cannot reach Hermes; the "
                         "file must list both the equity and the token feed.")
    args = ap.parse_args()

    if args.from_file:
        global FROM_FILE
        try:
            FROM_FILE = json.loads(args.from_file.read_text())
        except (OSError, json.JSONDecodeError) as exc:
            print(f"{args.from_file}: {exc}", file=sys.stderr)
            return 1
        print(f"reading {args.from_file} instead of querying Hermes\n")

    if args.self_test:
        return self_test()

    symbols = args.symbols or DEFAULT_SYMBOLS
    resolved, all_problems = [], []

    for sym in symbols:
        pair, problems = resolve(sym)
        all_problems.extend(problems)
        if pair:
            resolved.append(pair)
            print(f"  ok    {sym:<6} equity {pair['equity']['feed_id'][:16]}…  "
                  f"token {pair['token']['feed_id'][:16]}…")
        else:
            print(f"  SKIP  {sym:<6} incomplete pair")

    if all_problems:
        print("\nproblems:", file=sys.stderr)
        for p in all_problems:
            print(f"  - {p}", file=sys.stderr)

    if not resolved:
        print("\nNo symbol had both legs. Nothing written.", file=sys.stderr)
        print("Try other tickers, or check the notes above for close matches.", file=sys.stderr)
        return 1

    doc = {
        "_comment": (
            "Generated by scripts/fetch-feed-ids.py. Do not hand-edit: a wrong "
            "feed id publishes a real price for the wrong asset."
        ),
        "generated_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "source": HERMES + FEEDS_PATH,
        "symbols": resolved,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(doc, indent=2) + "\n")
    print(f"\nwrote {args.out} — {len(resolved)}/{len(symbols)} symbols complete")
    if len(resolved) < 3:
        print("Fewer than three complete pairs; consider adding symbols.", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())

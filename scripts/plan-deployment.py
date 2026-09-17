#!/usr/bin/env python3
"""Derive every address a stock deployment uses, and write both files that need them.

Two files have to agree about a dozen addresses per symbol: the txtx runbook
that creates the accounts, and `deployments.json` that the keeper, the
dashboard and stock-check read. If they disagree the deployment does not fail
-- it creates accounts at one set of addresses and then reads a different set,
and every reader reports "account not found" while the runbook reports success.

So neither file is written by hand. Both come from this script, from one
derivation, so they cannot drift apart.

    python3 scripts/fetch-feed-ids.py AAPL TSLA NVDA      # first: the feed ids
    python3 scripts/plan-deployment.py \\
        --svi-core <PROGRAM_ID> --adapter <PROGRAM_ID>

    python3 scripts/plan-deployment.py --self-test        # no network, no files

The program ids come from `anchor keys list` in each workspace, after
`anchor keys sync`. Everything else is derived.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from solana_pda import b58decode, is_on_curve, pda  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
FEEDS_IN = ROOT / "config" / "pyth-feeds.json"
DEPLOYMENTS_OUT = ROOT / "deployments.json"
RUNBOOK_OUT = ROOT / "programs/svi-stock-adapter/runbooks/publish/main.tx"

# Pyth's push oracle, which owns the sponsored price accounts. From
# `pyth_solana_receiver_sdk::PYTH_PUSH_ORACLE_ID` (default feature set) -- read
# out of the vendored crate source, not recalled.
PYTH_PUSH_ORACLE = "pythWSnswVUd12oZpeFP8e9CVaEqJg25g1Vtc2biRsT"

SYMBOL_LEN = 8

# ---------------------------------------------------------------- thresholds
#
# These are the numbers that decide when a flag is raised, and they are the
# whole product: set them wrong and either every quote is flagged or none ever
# is. Each is justified rather than round.

# US equities trade 09:30-16:00 ET. Pyth publishes an equity price every few
# seconds while the market is open, so a price older than 15 minutes during a
# session would itself be a fault -- but the same 15 minutes is reached a
# quarter of an hour after the close, which is exactly when we want to start
# saying MARKET_CLOSED. One threshold, both jobs.
MARKET_CLOSED_SECS = 15 * 60

# A normal overnight gap is ~17.5h; a weekend is ~65h; a long weekend with a
# Monday holiday is ~89h. REFERENCE_STALE means "older than any ordinary
# closure", so it sits above all of those at 4 days.
REFERENCE_STALE_SECS = 4 * 24 * 3600

# Past 8 days, no sequence of holidays explains it and the feed is broken.
# Publishing then would be publishing a number with no evidence behind it.
REFERENCE_MAX_AGE_SECS = 8 * 24 * 3600

# The token trades continuously, so its feed has no excuse for a gap.
TOKEN_STALE_SECS = 120
TOKEN_MAX_AGE_SECS = 30 * 60

# The premium this product exists to measure. Tokenized equities have traded
# multiple percent away from the underlying overnight; 2% is high enough not to
# fire on ordinary spread and low enough to catch a real dislocation.
MAX_DEVIATION_BPS = 200

# Pyth's confidence interval on a liquid equity is single-digit bps. 5% is not
# a tolerance for normal conditions -- it is a refusal threshold for an oracle
# that has stopped having an opinion.
MAX_CONF_BPS = 500

# ~750 slots is about five minutes at 400ms. A consumer reading a quote older
# than that is reading a number from before the last few candles.
MAX_AGE_SLOTS = 750

QUOTE_DECIMALS = 9
USD = 840
VALUE_TYPE_MARKET_SPOT = 2
VALUE_TYPE_REFERENCE_FAIR_VALUE = 7


def feed_id(name: str) -> bytes:
    return hashlib.sha256(name.encode()).digest()


def symbol_seed(symbol: str) -> bytes:
    """The config PDA seed: the ticker right-padded with zeros to 8 bytes."""
    raw = symbol.encode()
    if not raw or len(raw) > SYMBOL_LEN or not symbol.isalnum():
        raise ValueError(f"{symbol!r} is not 1-{SYMBOL_LEN} alphanumeric ASCII")
    return raw.ljust(SYMBOL_LEN, b"\0")


def sponsored_price_account(pyth_feed_id_hex: str, shard: int) -> str:
    """Where Pyth's push oracle keeps this feed's price account.

    Seeds are the shard id as two little-endian bytes, then the 32-byte feed
    id. A wrong address here is caught on-chain -- the adapter re-reads the
    feed id inside the account -- but it is caught as a failed transaction, so
    it is worth getting right up front.
    """
    return pda(
        PYTH_PUSH_ORACLE,
        [shard.to_bytes(2, "little"), bytes.fromhex(pyth_feed_id_hex)],
    )


def plan_symbol(symbol: str, feeds: dict, core: str, adapter: str, shard: int) -> dict:
    sym = symbol.lower()
    fair_id = feed_id(f"stock-{sym}-fair-value-v1")
    market_id = feed_id(f"stock-{sym}-market-v1")
    seed = symbol_seed(symbol)

    equity_hex = feeds["equity"]["feed_id"]
    token_hex = feeds["token"]["feed_id"]
    if equity_hex == token_hex:
        raise ValueError(f"{symbol}: both Pyth legs have the same feed id")

    return {
        "symbol": symbol,
        "svi_feed_ids": {"fair": fair_id.hex(), "market": market_id.hex()},
        "feed_ids": {"equity": equity_hex, "token": token_hex},
        "pyth_symbols": {
            "equity": feeds["equity"]["pyth_symbol"],
            "token": feeds["token"]["pyth_symbol"],
        },
        "config": pda(adapter, [b"stock-config", seed]),
        "adapter_authority": pda(adapter, [b"authority"]),
        "fair_descriptor": pda(core, [b"descriptor", fair_id]),
        "fair_quote": pda(core, [b"quote", fair_id]),
        "market_descriptor": pda(core, [b"descriptor", market_id]),
        "market_quote": pda(core, [b"quote", market_id]),
        "equity_price": sponsored_price_account(equity_hex, shard),
        "token_price": sponsored_price_account(token_hex, shard),
    }


def check_distinct(plan: list[dict]) -> list[str]:
    """Every derived account must be unique across the whole deployment.

    A collision means two feeds would write to one account, and the second
    write would look like an ordinary update rather than a bug.
    """
    problems, seen = [], {}
    for s in plan:
        for field in (
            "config",
            "fair_descriptor",
            "fair_quote",
            "market_descriptor",
            "market_quote",
        ):
            addr = s[field]
            where = f"{s['symbol']}.{field}"
            if addr in seen:
                problems.append(f"{where} collides with {seen[addr]} at {addr}")
            seen[addr] = where
        if s["fair_quote"] == s["market_quote"]:
            problems.append(f"{s['symbol']}: the two quotes are the same account")
        if s["equity_price"] == s["token_price"]:
            problems.append(f"{s['symbol']}: the two price accounts are the same")
        for field in ("config", "fair_quote", "market_quote"):
            if is_on_curve(b58decode(s[field])):
                problems.append(f"{s['symbol']}.{field} is not a PDA")
    return problems


def byte_array(raw: bytes) -> str:
    """A [u8; 32] literal, which is what txtx's IDL encoder wants for arrays."""
    rows = [raw[i : i + 12] for i in range(0, len(raw), 12)]
    body = ",\n        ".join(", ".join(str(b) for b in row) for row in rows)
    return "[\n        " + body + "\n    ]"


def render_runbook(plan: list[dict], generated_at: str) -> str:
    out: list[str] = []
    w = out.append

    w(f'''################################################################
# Deploy the tokenized-stock adapter and publish a quote pair per symbol.
#
# GENERATED by scripts/plan-deployment.py at {generated_at}. Do not hand-edit:
# deployments.json is generated from the same derivation in the same run, and
# editing one file makes the two disagree about addresses without either one
# failing.
#
#   cd programs/svi-core           && anchor build
#   cd programs/svi-stock-adapter  && anchor build
#   surfpool run publish --env devnet --unsupervised
#
# --env is REQUIRED. Signer files are named `signers.<env>.tx`, and txtx skips
# any file with three dot-components unless the middle one matches the selected
# environment. Without it the signers silently do not load and every action
# fails with "unable to resolve 'signer.payer'".
#
# PUBKEYS IN instruction_args, learned the hard way on the Hylo runbook:
#
#   action.*.program_id   is a Value::Addon of raw bytes despite being declared
#                         a string. MUST be wrapped in std::encode_base58.
#   variable.*.pda        is ALREADY a base58 string despite being declared an
#                         addon. Pass it through. Wrapping it makes
#                         encode_base58 hex-decode base58 and fail.
#   a [u8; 32] argument   must be an array of numbers; a hex string is
#                         rejected with "expected vec, found string".
#   a find_pda seed       must be the opposite: a hex string, because find_pda
#                         calls to_le_bytes on each element and a 32-number
#                         array blows past the 32-byte seed limit.
#
# So each feed id appears twice below, in both encodings. They are generated
# together and cannot drift.
################################################################

addon "svm" {{
    rpc_api_url = input.rpc_api_url
    network_id = input.network_id
}}

################################################################
# 1. Programs
################################################################

action "deploy_svi_core" "svm::deploy_program" {{
    description = "Deploy svi-core, the notice board"
    program = svm::get_program_from_anchor_project(
        "svi_core",
        "../svi-core/target/deploy/svi_core-keypair.json",
        "../svi-core/target/idl/svi_core.json",
        "../svi-core/target/deploy/svi_core.so"
    )
    authority = signer.authority
    payer = signer.payer
}}

action "deploy_adapter" "svm::deploy_program" {{
    description = "Deploy svi-stock-adapter, the calculator"
    program = svm::get_program_from_anchor_project("svi_stock_adapter")
    authority = signer.authority
    payer = signer.payer
    depends_on = [action.deploy_svi_core]
}}

################################################################
# 2. Shared values
#
# methodology_hash is zero until the spec document is frozen. A real
# deployment MUST set it: consumers compare it to detect a feed's definition
# changing under them, and zero means "no definition has been committed to".
################################################################

variable "methodology_hash" {{
    value = {byte_array(bytes(32))}
}}

variable "adapter_authority" {{
    value = svm::find_pda(action.deploy_adapter.program_id, ["authority"])
}}
''')

    for s in plan:
        sym = s["symbol"]
        lo = sym.lower()
        fair, market = s["svi_feed_ids"]["fair"], s["svi_feed_ids"]["market"]
        sym_bytes = symbol_seed(sym)

        w(f'''
################################################################
# {sym}
#
#   fair   feed id sha256("stock-{lo}-fair-value-v1")
#          from Pyth {s["pyth_symbols"]["equity"]}
#   market feed id sha256("stock-{lo}-market-v1")
#          from Pyth {s["pyth_symbols"]["token"]}
################################################################

variable "{lo}_fair_feed_id" {{
    value = {byte_array(bytes.fromhex(fair))}
}}
variable "{lo}_fair_seed" {{ value = "0x{fair}" }}

variable "{lo}_market_feed_id" {{
    value = {byte_array(bytes.fromhex(market))}
}}
variable "{lo}_market_seed" {{ value = "0x{market}" }}

variable "{lo}_symbol" {{
    value = {byte_array(sym_bytes)}
}}

variable "{lo}_equity_feed_id" {{
    value = {byte_array(bytes.fromhex(s["feed_ids"]["equity"]))}
}}
variable "{lo}_token_feed_id" {{
    value = {byte_array(bytes.fromhex(s["feed_ids"]["token"]))}
}}

variable "{lo}_config" {{
    value = svm::find_pda(action.deploy_adapter.program_id, ["stock-config", "0x{sym_bytes.hex()}"])
}}
variable "{lo}_fair_descriptor" {{
    value = svm::find_pda(action.deploy_svi_core.program_id, ["descriptor", variable.{lo}_fair_seed])
}}
variable "{lo}_fair_quote" {{
    value = svm::find_pda(action.deploy_svi_core.program_id, ["quote", variable.{lo}_fair_seed])
}}
variable "{lo}_market_descriptor" {{
    value = svm::find_pda(action.deploy_svi_core.program_id, ["descriptor", variable.{lo}_market_seed])
}}
variable "{lo}_market_quote" {{
    value = svm::find_pda(action.deploy_svi_core.program_id, ["quote", variable.{lo}_market_seed])
}}

# Pyth's sponsored price accounts for this symbol, shard 0. The adapter
# re-reads the feed id inside each one, so a wrong address fails the
# transaction rather than publishing another asset's price.
variable "{lo}_equity_price" {{ value = "{s["equity_price"]}" }}
variable "{lo}_token_price" {{ value = "{s["token_price"]}" }}

# value_type 7 is ReferenceFairValue: what the share is worth where it really
# trades. base_decimals 0 because the reference is priced per whole share.
action "init_{lo}_fair_feed" "svm::process_instructions" {{
    description = "Create the {sym} fair-value feed"
    instruction {{
        program_id = action.deploy_svi_core.program_id
        program_idl = action.deploy_svi_core.program_idl
        instruction_name = "initialize_feed"
        instruction_args = [{{
            feed_id             = variable.{lo}_fair_feed_id,
            adapter_program     = std::encode_base58(action.deploy_adapter.program_id),
            adapter_authority   = variable.adapter_authority.pda,
            adapter_config      = variable.{lo}_config.pda,
            base_mint           = "0x{"00" * 32}",
            quote_mint          = "0x{"00" * 32}",
            methodology_hash    = variable.methodology_hash,
            max_age_slots       = {MAX_AGE_SLOTS},
            quote_currency_code = {USD},
            value_type          = {VALUE_TYPE_REFERENCE_FAIR_VALUE},
            base_decimals       = 0,
            quote_decimals      = {QUOTE_DECIMALS}
        }}]
        authority {{ public_key = signer.authority.public_key }}
        descriptor {{ public_key = variable.{lo}_fair_descriptor.pda }}
        quote {{ public_key = variable.{lo}_fair_quote.pda }}
    }}
    signers = [signer.authority]
    depends_on = [action.deploy_adapter]
}}

# value_type 2 is MarketSpot: what the token trades at on Solana right now.
action "init_{lo}_market_feed" "svm::process_instructions" {{
    description = "Create the {sym} market-price feed"
    instruction {{
        program_id = action.deploy_svi_core.program_id
        program_idl = action.deploy_svi_core.program_idl
        instruction_name = "initialize_feed"
        instruction_args = [{{
            feed_id             = variable.{lo}_market_feed_id,
            adapter_program     = std::encode_base58(action.deploy_adapter.program_id),
            adapter_authority   = variable.adapter_authority.pda,
            adapter_config      = variable.{lo}_config.pda,
            base_mint           = "0x{"00" * 32}",
            quote_mint          = "0x{"00" * 32}",
            methodology_hash    = variable.methodology_hash,
            max_age_slots       = {MAX_AGE_SLOTS},
            quote_currency_code = {USD},
            value_type          = {VALUE_TYPE_MARKET_SPOT},
            base_decimals       = 0,
            quote_decimals      = {QUOTE_DECIMALS}
        }}]
        authority {{ public_key = signer.authority.public_key }}
        descriptor {{ public_key = variable.{lo}_market_descriptor.pda }}
        quote {{ public_key = variable.{lo}_market_quote.pda }}
    }}
    signers = [signer.authority]
    depends_on = [action.init_{lo}_fair_feed]
}}

# Every threshold the adapter applies, written on-chain so a third party can
# read the rules that produced a quote instead of trusting a description.
action "init_{lo}_symbol" "svm::process_instructions" {{
    description = "Configure {sym}: which feeds are authoritative, and when to flag"
    instruction {{
        program_id = action.deploy_adapter.program_id
        program_idl = action.deploy_adapter.program_idl
        instruction_name = "initialize_symbol"
        instruction_args = [{{
            symbol                 = variable.{lo}_symbol,
            svi_core_program       = std::encode_base58(action.deploy_svi_core.program_id),
            fair_descriptor        = variable.{lo}_fair_descriptor.pda,
            fair_quote             = variable.{lo}_fair_quote.pda,
            market_descriptor      = variable.{lo}_market_descriptor.pda,
            market_quote           = variable.{lo}_market_quote.pda,
            equity_feed_id         = variable.{lo}_equity_feed_id,
            token_feed_id          = variable.{lo}_token_feed_id,
            market_closed_secs     = {MARKET_CLOSED_SECS},
            reference_stale_secs   = {REFERENCE_STALE_SECS},
            reference_max_age_secs = {REFERENCE_MAX_AGE_SECS},
            token_stale_secs       = {TOKEN_STALE_SECS},
            token_max_age_secs     = {TOKEN_MAX_AGE_SECS},
            max_deviation_bps      = {MAX_DEVIATION_BPS},
            max_conf_bps           = {MAX_CONF_BPS},
            methodology_hash       = variable.methodology_hash,
            base_decimals          = 0,
            min_verification_level = 1
        }}]
        authority {{ public_key = signer.authority.public_key }}
        config {{ public_key = variable.{lo}_config.pda }}
        adapter_authority {{ public_key = variable.adapter_authority.pda }}
    }}
    signers = [signer.authority]
    depends_on = [action.init_{lo}_market_feed]
}}

# Signed by the payer, not the authority: cranking is permissionless, and the
# keeper cannot influence the value that gets written.
action "refresh_{lo}" "svm::process_instructions" {{
    description = "Publish {sym}'s fair value and market price, in one transaction"
    instruction {{
        program_id = action.deploy_adapter.program_id
        program_idl = action.deploy_adapter.program_idl
        instruction_name = "refresh_stock"
        instruction_args = []

        payer {{ public_key = signer.payer.public_key }}
        config {{ public_key = variable.{lo}_config.pda }}
        equity_price {{ public_key = variable.{lo}_equity_price }}
        token_price {{ public_key = variable.{lo}_token_price }}
        fair_descriptor {{ public_key = variable.{lo}_fair_descriptor.pda }}
        fair_quote {{
            public_key = variable.{lo}_fair_quote.pda
            is_writable = true
        }}
        market_descriptor {{ public_key = variable.{lo}_market_descriptor.pda }}
        market_quote {{
            public_key = variable.{lo}_market_quote.pda
            is_writable = true
        }}
        adapter_authority {{ public_key = variable.adapter_authority.pda }}
        svi_core_program {{ public_key = action.deploy_svi_core.program_id }}
    }}
    signers = [signer.payer]
    depends_on = [action.init_{lo}_symbol]
}}

output "{lo}_fair_quote" {{
    description = "{sym} fair value -- what the real share is worth"
    value = variable.{lo}_fair_quote.pda
}}
output "{lo}_market_quote" {{
    description = "{sym} market price -- what the token trades at on Solana"
    value = variable.{lo}_market_quote.pda
}}
''')

    return "".join(out)


def render_deployments(plan: list[dict], core: str, adapter: str, cluster: str,
                       generated_at: str) -> dict:
    return {
        "_comment": [
            "GENERATED by scripts/plan-deployment.py. Do not hand-edit -- the",
            "txtx runbook is generated from the same derivation in the same run,",
            "and editing one file makes the two disagree about addresses without",
            "either one failing.",
            "",
            "equity_price / token_price are Pyth's sponsored price accounts. The",
            "adapter re-checks the feed id inside each one on every refresh, so a",
            "wrong address here fails loudly (PythFeedMismatch, 6003) rather than",
            "publishing the wrong asset's price.",
        ],
        "cluster": cluster,
        "generated_at": generated_at,
        "programs": {"svi_core": core, "svi_stock_adapter": adapter},
        "stocks": {
            s["symbol"]: {
                "feed_ids": s["feed_ids"],
                "pyth_symbols": s["pyth_symbols"],
                "svi_feed_ids": s["svi_feed_ids"],
                "config": s["config"],
                "adapter_authority": s["adapter_authority"],
                "fair_descriptor": s["fair_descriptor"],
                "fair_quote": s["fair_quote"],
                "market_descriptor": s["market_descriptor"],
                "market_quote": s["market_quote"],
                "equity_price": s["equity_price"],
                "token_price": s["token_price"],
            }
            for s in plan
        },
    }


def self_test() -> int:
    core = "H6495aW1Cxyoz2R7MuYVHF3Pu3FumFE9cZwKSJCR8oHH"
    adapter = "3RmdZomoBXkWwGwvdjWK8ELYB4XeqHxTmrzedcmucHxq"
    feeds = {
        "equity": {"pyth_symbol": "Equity.US.AAPL/USD", "feed_id": "aa" * 32},
        "token": {"pyth_symbol": "Crypto.AAPLX/USD", "feed_id": "bb" * 32},
    }
    a = plan_symbol("AAPL", feeds, core, adapter, 0)
    b = plan_symbol("TSLA", feeds, core, adapter, 0)

    assert a["fair_quote"] != a["market_quote"], "one symbol, two distinct quotes"
    assert a["fair_quote"] != b["fair_quote"], "two symbols, distinct quotes"
    assert a["config"] != b["config"], "two symbols, distinct configs"
    assert a["adapter_authority"] == b["adapter_authority"], "one shared authority"
    assert not check_distinct([a, b]), check_distinct([a, b])

    # Derivation is a pure function of its inputs.
    assert plan_symbol("AAPL", feeds, core, adapter, 0) == a

    # A different shard is a different price account.
    assert plan_symbol("AAPL", feeds, core, adapter, 1)["equity_price"] != a["equity_price"]

    # The config seed is the padded ticker, and padding is only at the end.
    assert symbol_seed("AAPL") == b"AAPL\0\0\0\0"
    assert symbol_seed("GOOGL") == b"GOOGL\0\0\0"
    for bad in ("", "TOOLONGSYM", "AA PL", "AA.PL"):
        try:
            symbol_seed(bad)
        except ValueError:
            pass
        else:
            raise AssertionError(f"{bad!r} should have been rejected")

    # Identical Pyth legs are a config that can only ever report zero drift.
    same = {"equity": dict(feeds["equity"]), "token": dict(feeds["token"])}
    same["token"]["feed_id"] = same["equity"]["feed_id"]
    try:
        plan_symbol("AAPL", same, core, adapter, 0)
    except ValueError:
        pass
    else:
        raise AssertionError("identical feed ids should have been rejected")

    # The generated runbook must name every account the refresh needs, in the
    # encodings txtx accepts.
    rb = render_runbook([a], "1970-01-01T00:00:00+00:00")
    for needle in (
        'instruction_name = "refresh_stock"',
        'instruction_name = "initialize_symbol"',
        "value_type          = 7",
        "value_type          = 2",
        "std::encode_base58(action.deploy_adapter.program_id)",
        "variable.aapl_fair_quote.pda",
        "is_writable = true",
        'variable "aapl_fair_seed" { value = "0x',
    ):
        assert needle in rb, f"generated runbook is missing {needle!r}"
    assert rb.count("is_writable = true") == 2, "both quotes must be writable"
    assert "std::encode_base58(variable." not in rb, "a .pda must not be wrapped"

    # Brackets must balance. Without this a mangled array literal renders as
    # something txtx rejects only at deploy time, in front of an audience.
    for opener, closer in (("[", "]"), ("{", "}")):
        assert rb.count(opener) == rb.count(closer), (
            f"generated runbook has {rb.count(opener)} {opener} "
            f"and {rb.count(closer)} {closer}"
        )
    # The symbol literal is the padded ticker as 8 numbers, on one line.
    assert "65, 65, 80, 76, 0, 0, 0, 0\n    ]" in rb, "symbol array is malformed"

    d = render_deployments([a, b], core, adapter, "devnet", "1970-01-01T00:00:00+00:00")
    assert set(d["stocks"]) == {"AAPL", "TSLA"}
    json.dumps(d)  # must be serialisable

    print("plan-deployment self-test ok: derivation, collisions, both renderers")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--svi-core", help="svi-core program id (anchor keys list)")
    ap.add_argument("--adapter", help="svi-stock-adapter program id")
    ap.add_argument("--cluster", default="devnet")
    ap.add_argument("--pyth-shard", type=int, default=0)
    ap.add_argument("--feeds", type=Path, default=FEEDS_IN)
    ap.add_argument("--out", type=Path, default=DEPLOYMENTS_OUT)
    ap.add_argument("--runbook", type=Path, default=RUNBOOK_OUT)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    if not args.svi_core or not args.adapter:
        print("--svi-core and --adapter are required. Get them with `anchor keys list`",
              file=sys.stderr)
        return 2
    for label, pk in (("--svi-core", args.svi_core), ("--adapter", args.adapter)):
        if len(b58decode(pk)) != 32:
            print(f"{label} {pk!r} is not a 32-byte address", file=sys.stderr)
            return 2

    if not args.feeds.exists():
        print(f"{args.feeds} does not exist. Run scripts/fetch-feed-ids.py first.",
              file=sys.stderr)
        return 1
    doc = json.loads(args.feeds.read_text())

    plan = [
        plan_symbol(e["symbol"], e, args.svi_core, args.adapter, args.pyth_shard)
        for e in doc["symbols"]
    ]
    if not plan:
        print(f"{args.feeds} lists no symbols.", file=sys.stderr)
        return 1

    problems = check_distinct(plan)
    if problems:
        print("address collisions -- nothing written:", file=sys.stderr)
        for p in problems:
            print(f"  - {p}", file=sys.stderr)
        return 1

    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    args.out.write_text(
        json.dumps(
            render_deployments(plan, args.svi_core, args.adapter, args.cluster, now),
            indent=2,
        )
        + "\n"
    )
    args.runbook.parent.mkdir(parents=True, exist_ok=True)
    args.runbook.write_text(render_runbook(plan, now))

    for s in plan:
        print(f"  {s['symbol']:<6} fair {s['fair_quote']}")
        print(f"  {'':<6} mkt  {s['market_quote']}")
    print(f"\nwrote {args.out}")
    print(f"wrote {args.runbook}")
    print(f"\n{len(plan)} symbol(s). Next: surfpool run publish --env {args.cluster} --unsupervised")
    return 0


if __name__ == "__main__":
    sys.exit(main())

"""Base58 and `find_program_address`, with no dependencies.

The deploy planner has to derive the same PDAs the on-chain programs derive.
Getting one wrong does not fail visibly at planning time — it writes a
plausible-looking address into `deployments.json` and the failure surfaces
later as an Anchor seeds-constraint error with no hint about which of a dozen
addresses was wrong. So the derivation is reproduced here exactly, and
`--self-test` checks it against known-good vectors before anything is written.

Pure stdlib on purpose: this must run on an operator's laptop during a
deployment, at which point `pip install` is not a step anybody wants.
"""

from __future__ import annotations

import hashlib

_B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"

# ed25519 field and curve parameters, needed only to answer one question:
# "is this 32-byte string a valid curve point?" A program-derived address must
# NOT be one, which is what makes it unsignable.
_P = 2**255 - 19
_D = -121665 * pow(121666, _P - 2, _P) % _P


def b58encode(raw: bytes) -> str:
    n = int.from_bytes(raw, "big")
    out = ""
    while n:
        n, r = divmod(n, 58)
        out = _B58[r] + out
    # Every leading zero byte is one leading '1'. All-zero input is therefore
    # all '1's and nothing else -- appending a fallback digit here would make
    # the system program 33 characters long.
    return "1" * (len(raw) - len(raw.lstrip(b"\0"))) + out


def b58decode(s: str) -> bytes:
    n = 0
    for ch in s:
        i = _B58.find(ch)
        if i < 0:
            raise ValueError(f"{ch!r} is not a base58 character")
        n = n * 58 + i
    body = n.to_bytes((n.bit_length() + 7) // 8, "big") if n else b""
    return b"\0" * (len(s) - len(s.lstrip("1"))) + body


def _recover_x(y: int, sign: int) -> int | None:
    """The x coordinate of the curve point with this y, or None if there is none."""
    if y >= _P:
        return None
    y2 = y * y % _P
    u = (y2 - 1) % _P
    v = (_D * y2 + 1) % _P
    # x = u * v^3 * (u * v^7)^((p-5)/8), the standard ed25519 square root.
    uv3 = u * pow(v, 3, _P) % _P
    uv7 = u * pow(v, 7, _P) % _P
    x = uv3 * pow(uv7, (_P - 5) // 8, _P) % _P
    if (v * x * x - u) % _P == 0:
        pass
    elif (v * x * x + u) % _P == 0:
        x = x * pow(2, (_P - 1) // 4, _P) % _P
    else:
        return None
    if x == 0 and sign:
        return None
    if x % 2 != sign:
        x = _P - x
    return x


def is_on_curve(point: bytes) -> bool:
    if len(point) != 32:
        return False
    y = int.from_bytes(point, "little")
    sign = y >> 255
    return _recover_x(y & ((1 << 255) - 1), sign) is not None


_PDA_MARKER = b"ProgramDerivedAddress"


def create_program_address(seeds: list[bytes], program_id: bytes) -> bytes | None:
    for s in seeds:
        if len(s) > 32:
            raise ValueError(f"seed of {len(s)} bytes exceeds the 32-byte limit")
    h = hashlib.sha256(b"".join(seeds) + program_id + _PDA_MARKER).digest()
    return None if is_on_curve(h) else h


def find_program_address(seeds: list[bytes], program_id: bytes) -> tuple[bytes, int]:
    """The canonical PDA: the highest bump that lands off the curve."""
    for bump in range(255, -1, -1):
        addr = create_program_address([*seeds, bytes([bump])], program_id)
        if addr is not None:
            return addr, bump
    raise ValueError("no bump produced an off-curve address (probability ~2^-256)")


def pda(program_id_b58: str, seeds: list[bytes]) -> str:
    addr, _ = find_program_address(seeds, b58decode(program_id_b58))
    return b58encode(addr)


SYSTEM_PROGRAM = "11111111111111111111111111111111"


def self_test() -> int:
    # Base58 round-trips, including the leading-zero case that a naive
    # implementation silently drops.
    assert b58encode(b"\0" * 32) == "1" * 32
    assert b58decode("1" * 32) == b"\0" * 32
    assert len(b58decode(SYSTEM_PROGRAM)) == 32

    known = "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ"
    assert b58encode(b58decode(known)) == known, "base58 is not a round trip"
    assert len(b58decode(known)) == 32

    # A real on-curve key must be rejected as a PDA, and a real PDA accepted.
    assert is_on_curve(b58decode(known)), "a program id is an ordinary ed25519 key"

    # The one vector that matters: this exact derivation produced the quote
    # account that was published to on a mainnet fork on 2026-09-10, and the
    # address below is the one the transaction actually wrote
    # (docs/validation/2026-09-10-xsol-nav-mainnet.md). It pins the seed order,
    # the sha256 feed-id convention and the downward bump search together, and
    # it was confirmed on-chain rather than recalled.
    core = "H6495aW1Cxyoz2R7MuYVHF3Pu3FumFE9cZwKSJCR8oHH"
    feed_id = hashlib.sha256(b"hylo-xsol-nav-v1").digest()
    assert (
        feed_id.hex()
        == "6ab826e3a5d80bc1cab3804748ea4f4d27bb6dd433e2d3bae1bc5b3f4d895c86"
    ), feed_id.hex()
    quote = pda(core, [b"quote", feed_id])
    assert quote == "G28Ba5F9X71Pih2SNobXyU98tjQdga1Ztbzqg3PU4rWj", quote
    assert not is_on_curve(b58decode(quote)), "a PDA must be off the curve"

    # Seed order is part of the address: swapping the two changes it.
    assert pda(core, [feed_id, b"quote"]) != quote

    # A one-byte seed, the shape the Pyth shard and bump seeds use.
    assert len(b58decode(pda(core, [b"treasury", bytes([0])]))) == 32

    # Determinism, and that the bump is actually searched downward.
    a1, b1 = find_program_address([b"authority"], b58decode(known))
    a2, b2 = find_program_address([b"authority"], b58decode(known))
    assert (a1, b1) == (a2, b2)
    assert 0 <= b1 <= 255

    try:
        create_program_address([b"x" * 33], b58decode(known))
    except ValueError:
        pass
    else:
        raise AssertionError("an over-long seed must be rejected, not truncated")

    print("solana_pda self-test ok: base58, curve check, PDA derivation")
    return 0


if __name__ == "__main__":
    raise SystemExit(self_test())

#!/usr/bin/env bash
#
# FALLBACK ONLY. Prefer Surfpool, which forks mainnet on demand and needs none
# of the account enumeration or slot warping below:
#
#     surfpool start
#     surfpool run publish        # programs/svi-hylo-adapter/runbooks/publish
#
# This script exists for anyone without Surfpool installed. It starts a stock
# solana-test-validator carrying Hylo's real mainnet accounts, cloned by hand,
# so the adapter executes against the same bytes mainnet has.
#
#   ./scripts/localnet.sh 'https://your-mainnet-rpc'
#
# Then, in another shell:
#   ./scripts/deploy-local.sh
#   cargo run --manifest-path tools/svi-e2e/Cargo.toml
#
set -euo pipefail

MAINNET_RPC="${1:-${RPC_URL:-https://api.mainnet-beta.solana.com}}"
LEDGER="${LEDGER_DIR:-.localnet-ledger}"

command -v solana-test-validator >/dev/null || {
  echo "solana-test-validator not found. Install the Solana CLI first." >&2
  exit 1
}

echo "Resolving the accounts to clone (from the SDK, not hardcoded)..."
mapfile -t ACCOUNTS < <(cargo run -q --manifest-path tools/nav-check/Cargo.toml -- --addresses)
[ "${#ACCOUNTS[@]}" -ge 3 ] || { echo "expected at least 3 accounts, got ${#ACCOUNTS[@]}" >&2; exit 1; }

CLONE_ARGS=()
for a in "${ACCOUNTS[@]}"; do
  echo "  clone $a"
  CLONE_ARGS+=(--clone "$a")
done

# A fresh validator starts at slot 0, epoch 0 — but Hylo's TotalSolCache is
# stamped with the epoch it was computed in. Without warping, hylo-core
# correctly refuses with TotalSolCacheOutdated and nothing publishes. That
# refusal is right, and worth seeing once: run with WARP=0 to observe it.
#
# The default epoch schedule is 432,000 slots, so warping to the mainnet slot
# reproduces the mainnet epoch.
WARP_ARGS=()
if [ "${WARP:-1}" = "1" ]; then
  SLOT="$(curl -sS -m 20 -X POST -H 'content-type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getSlot"}' "$MAINNET_RPC" \
    | sed -n 's/.*"result":\([0-9]*\).*/\1/p')"
  [ -n "$SLOT" ] || { echo "could not read the current mainnet slot" >&2; exit 1; }
  echo "Warping to mainnet slot $SLOT (epoch $((SLOT / 432000)))"
  WARP_ARGS=(--warp-slot "$SLOT")
else
  echo "WARP=0 — the validator stays at epoch 0, so the adapter should REFUSE"
  echo "         to publish with TotalSolCacheOutdated. That is correct behaviour."
fi

rm -rf "$LEDGER"
exec solana-test-validator \
  --reset \
  --ledger "$LEDGER" \
  --url "$MAINNET_RPC" \
  "${CLONE_ARGS[@]}" \
  "${WARP_ARGS[@]}"

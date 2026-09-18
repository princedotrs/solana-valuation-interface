#!/usr/bin/env bash
#
# Deploy svi-core and svi-hylo-adapter to the local validator.
# Run scripts/localnet.sh in another shell first.
set -euo pipefail

RPC="${RPC:-http://127.0.0.1:8899}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for p in svi-core svi-hylo-adapter; do
  SO="$ROOT/programs/$p/target/deploy/${p//-/_}.so"
  [ -f "$SO" ] || { echo "missing $SO — run 'anchor build' in programs/$p" >&2; exit 1; }
  printf '%-20s %s bytes\n' "$p" "$(wc -c < "$SO" | tr -d ' ')"
done

solana config set --url "$RPC" >/dev/null
solana airdrop 10 >/dev/null 2>&1 || true

for p in svi-core svi-hylo-adapter; do
  DIR="$ROOT/programs/$p"
  echo "deploying $p..."
  ( cd "$DIR" && anchor deploy --provider.cluster "$RPC" )
done

echo
echo "Deployed. Now run:"
echo "  cargo run --manifest-path tools/svi-e2e/Cargo.toml"

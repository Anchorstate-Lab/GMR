#!/bin/sh
set -eu

cd "$(dirname "$0")/../.."
cargo build -p gmr-node

for made in libgmr_node.dylib libgmr_node.so gmr_node.dll; do
  if [ -f "target/debug/$made" ]; then
    cp "target/debug/$made" "domains/node/gmr.node"
    GMR_ADDON="$PWD/domains/node/gmr.node" node --test domains/node/test/*.mjs
    exit $?
  fi
done

echo "node: cargo built no addon under target/debug" >&2
exit 1

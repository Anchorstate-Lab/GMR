#!/bin/sh
set -eu

cd "$(dirname "$0")/../.."
cargo build --release -p gmr-node

for made in libgmr_node.dylib libgmr_node.so gmr_node.dll; do
  if [ -f "target/release/$made" ]; then
    cp "target/release/$made" "domains/node/gmr.node"
    GMR_ADDON="$PWD/domains/node/gmr.node" node domains/node/bench/latency.mjs
    exit $?
  fi
done

echo "node: cargo built no addon under target/release" >&2
exit 1

#!/bin/sh
set -eu

cd "$(dirname "$0")/../.."
cargo build --release -p gmr-node

for made in libgmr_node.dylib libgmr_node.so gmr_node.dll; do
  if [ -f "target/release/$made" ]; then
    cp "target/release/$made" "console/node/gmr.node"
    GMR_ADDON="$PWD/console/node/gmr.node" node console/node/bench/latency.mjs
    exit $?
  fi
done

echo "node: cargo built no addon under target/release" >&2
exit 1

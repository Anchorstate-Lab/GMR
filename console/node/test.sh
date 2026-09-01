#!/bin/sh
set -eu

cd "$(dirname "$0")/../.."
cargo build -p gmr-node

for made in libgmr_node.dylib libgmr_node.so gmr_node.dll; do
  if [ -f "target/debug/$made" ]; then
    cp "target/debug/$made" "console/node/gmr.node"
    GMR_ADDON="$PWD/console/node/gmr.node" node --test console/node/test/*.mjs
    exit $?
  fi
done

echo "node: cargo built no addon under target/debug" >&2
exit 1

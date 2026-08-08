#!/bin/sh
# **Invariants that types and tests cannot express – the only home they have.**
# This script is the CI gate, as is.
#
#   sh gate.sh
#
# An invariant is a relation that holds once and must hold forever. It does not
# require anyone to receive a signal – so it should not live in prose, comments,
# or memory, because those three only take effect when a human reads them, and
# at the moment an invariant is broken nothing moves. Invariants have three
# homes, ordered by their ability to express them:
#
#   types        give priority: if a value is stored only once, it cannot diverge
#   tests        can be decided by Rust, live in `cargo test`, adjacent to code
#   tools/gate.py cross-package dependency facts, file-level textual discipline –
#                the first two cannot reach here.
#
# This script itself only runs cargo's own build/lint/test steps directly, in
# shell, because there is nothing to modularize there. Everything else – cross-
# package Topology and file-level Discipline – lives in tools/gate.py, which
# this script simply calls.
#
# It inspects the **source tree**, and never touches any anchor. Anchors report
# state that requires a human to look – not a build failure. If the gate judges
# them, semantics would be moved into tooling. Invariants about **runtime storage**
# belong to `gmr check`, not here.
set -e
cd "$(dirname "$0")"

echo "── clean"
cargo clean

echo "── fmt"
cargo fmt --all --check

echo "── clippy (-D warnings)"
cargo clippy --workspace --all-targets -- -D warnings

echo "── test"
cargo test --workspace

echo
echo "══ Topology + Discipline"
python3 tools/gate.py

echo
echo "gate: all invariants green"

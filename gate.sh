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
#   types       give priority: if a value is stored only once, it cannot diverge
#   tests       can be decided by Rust, live in `cargo test`, adjacent to code
#   gate.sh     cross‑package dependency facts, file‑level textual discipline –
#               the first two cannot reach here.
#
# This file has only two sections: Topology and Discipline. For a new invariant,
# first ask whether the first two homes can take it; only if not, it belongs here.
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
echo "══ Topology"

# Purity is not carried by trait splits (pure/impure), but by crate boundaries.
# However, the dependency list is an **external check**, not a type constraint –
# if not checked here, no one will.
echo "── pure roots: zero workspace dependencies"
for c in gmr-core gmr-expr; do
  if cargo tree -p "$c" --edges normal --prefix none | tail -n +2 | grep -qE '^gmr-'; then
    echo "gate: $c has workspace dependencies – it is no longer a pure root" >&2
    exit 1
  fi
done

echo "── dependency forbidden zones (only architecture.toml holds the list, no per‑crate copies)"
python3 - <<'FORBIDDEN' || exit 1
import tomllib, subprocess, sys
arch = tomllib.load(open("architecture.toml", "rb"))
bad = []
errs = []
for m in arch["member"]:
    if m.get("kind") != "package":
        continue
    keys = (arch["layer"][m["layer"]].get("forbidden", [])
            + m.get("forbidden", []) + m.get("forbidden_default", []))
    banned = {n for k in keys for n in arch["libs"][k]}
    if not banned:
        continue
    r = subprocess.run(["cargo", "tree", "--edges", "normal,no-proc-macro",
                        "--manifest-path", f"{m['path']}/Cargo.toml", "--prefix", "none"],
                       capture_output=True, text=True)
    if r.returncode:
        errs.append(f"{m['name']}: cannot resolve {m['path']}/Cargo.toml — "
                     "architecture.toml's path is stale")
        continue
    deps = {l.split()[0] for l in r.stdout.splitlines()[1:] if l.strip()}
    bad += [f"{m['name']} -> {d}" for d in sorted(deps & banned)]
if errs:
    print("gate: forbidden check cannot reach these members", *errs, sep="\n  ", file=sys.stderr)
    sys.exit(1)
if bad:
    print("gate: forbidden dependencies violated", *bad, sep="\n  ", file=sys.stderr)
    sys.exit(1)
FORBIDDEN

echo "── layering: lower layers must not depend on upper ones"
python3 - <<'LAYERS' || exit 1
import json, pathlib, subprocess, sys
LAYER = {"crates": 0, "batteries": 1, "domains": 2}
pkgs, edges, seen = {}, [], set()
for top in LAYER:
    for man in sorted(pathlib.Path(top).glob("**/Cargo.toml")):
        if "target" in man.parts:
            continue
        r = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1",
                            "--manifest-path", str(man)], capture_output=True, text=True)
        if r.returncode:
            print(f"gate: cannot read {man}", file=sys.stderr)
            sys.exit(1)
        for p in json.loads(r.stdout)["packages"]:
            if p["id"] in seen:
                continue
            seen.add(p["id"])
            lay = next((k for k in LAYER if f"/{k}/" in p["manifest_path"]), None)
            if lay is None:
                continue
            pkgs[p["name"]] = lay
            edges += [(p["name"], d["name"]) for d in p["dependencies"]
                      if d.get("path") and d.get("kind") is None]
bad = [f"{a}({pkgs[a]}) -> {b}({pkgs[b]})" for a, b in edges
       if a in pkgs and b in pkgs and LAYER[pkgs[b]] > LAYER[pkgs[a]]]
if bad:
    print("gate: lower layer depends on upper layer", *bad, sep="\n  ", file=sys.stderr)
    sys.exit(1)
LAYERS

echo "── base layer must not ship any concrete implementation"
if cargo tree -p gmr-probe --edges normal --prefix none | tail -n +2 |
   grep -qiE '^(tokio|reqwest|hyper)'; then
  echo "gate: gmr-probe pulls in a transport implementation – it should be only a contract" >&2
  exit 1
fi
if cargo tree -p gmr-store --edges normal --prefix none | tail -n +2 |
   grep -qiE '^(sqlx|rusqlite|libsqlite3|postgres|tokio-postgres)'; then
  echo "gate: gmr-store drags in a database implementation by default" >&2
  exit 1
fi

echo "── base layer must not produce any binary"
if cargo metadata --no-deps --format-version 1 |
   grep -oE '"name":"gmr[^"]*","[^}]*"kind":\["bin"\]' | grep -q .; then
  echo "gate: a binary appeared in the base layer – assembly belongs to domains" >&2
  exit 1
fi

echo "── facade: only re‑exports"
if grep -qE '^pub (fn|struct|enum|trait|const|type) ' crates/gmr/src/lib.rs; then
  echo "gate: facade contains a definition – it becomes a seventh package, and one without a guardian" >&2
  exit 1
fi
cargo build -p gmr --no-default-features

echo
echo "══ Discipline"

# Comments and memory each keep a copy, and they inevitably diverge. Memory is
# guarded by anchors, comments are not. The clean list only grows: after cleaning
# a package, add one line to CLEAN. That line is a receipt of a real cleaning.
# We do not diff against a base ref – diff needs a reference, which is state;
# the clean list is zero‑state and monotonic.
#
# EXEMPT is the second exception listed in CLAUDE.md: clap’s `///` is the body of
# --help, a user‑visible string that happens to use comment syntax. The first
# exception `//!` is about “what this file is” and is directly allowed in the rule.
echo "── no comments in the clean zones"
python3 - <<'COMMENTS' || exit 1
import pathlib, subprocess, sys

CLEAN = ["crates/gmr-core", "crates/gmr-content", "crates/gmr", "crates/gmr-probe",
         "batteries/survey", "domains/coding/extract"]
EXEMPT = ["domains/coding/cli/src/cli.rs"]

files = subprocess.run(["git", "ls-files", "*.rs"],
                       capture_output=True, text=True, check=True).stdout.split()
bad = []
for f in files:
    if f in EXEMPT or not any(f.startswith(d + "/") for d in CLEAN):
        continue
    for n, line in enumerate(pathlib.Path(f).read_text().splitlines(), 1):
        s = line.lstrip()
        if s.startswith("//") and not s.startswith("//!"):
            bad.append(f"{f}:{n}  {s[:60]}")
if bad:
    print("gate: comments found in clean zones – say it with an anchor, not a comment",
          *bad, sep="\n  ", file=sys.stderr)
    sys.exit(1)
COMMENTS

echo
echo "gate: all invariants green"
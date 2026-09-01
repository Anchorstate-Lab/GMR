#!/bin/sh
# The acceptance criterion, in one sentence:
#
#   An argument an agent makes must be traceable to a signal that really exists,
#   and when that signal moves the memory resting on it must come back to be
#   judged again -- in a stranger's repository, with no toolchain, whatever the
#   signal is and wherever the memory is kept.
#
# This file is the portal, and it holds only what a shell is the right tool for:
# building the tarball, installing it the way `dist/install.sh` does, and
# checking that nothing which belongs to *this* repository travels inside the
# package. Then it hands the shipped binary to tools/acceptance.py, which
# expands the promises over every world and every store.
#
# The promises themselves are not here on purpose. They need set equality, JSON
# structure, address round trips and mutation injection; a shell doing that
# grows `python3 -c` patches until it is a Python program with worse quoting.
# What lives where:
#
#   tools/accept/spec.py         the promises, in no domain's words
#   tools/accept/predicates.py   the six things a promise may assert
#   tools/accept/driver.py       the only file that knows what the CLI looks like
#   tools/accept/matrix.py       which cells must exist, and why one must not be code
#   tools/accept/mutations.py    proof the assertions still have teeth
#
# It is deliberately not part of gate.sh: gate.sh inspects the source tree and
# never touches an anchor, because a red anchor is a signal for a person, not a
# build failure. What runs here are the anchors of fixture repositories, which
# is test data, and asserting it is the job.
#
# The last line is a sentinel and CI greps for it with the step count. That is
# not decoration: this file was once truncated mid-heredoc by an editing pass,
# `sh` treated the unterminated `<<'EOF'` as delimited by end-of-file, and the
# run exited 0 having tested almost nothing for two days. `sh -n` does not catch
# that. The sentinel does. The Python half cannot fail that way -- a truncated
# module raises rather than passing -- so what tools/gate.py checks over there
# is the other way it could go hollow: a mutation whose anchor has drifted out
# of the source it was aimed at.
set -eu

root=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
bundle=$work/bundle
prefix=$work/prefix
repo=$work/stranger

fail() {
    echo
    echo "Acceptance failed: $1"
    [ $# -gt 1 ] && { echo "--- actual output ---"; echo "$2"; }
    exit 1
}

steps=0
step() { steps=$((steps + 1)); echo; echo "── $1"; }

# ── Shipping side: one binary. The extractor chain is inside it, and nothing
#    that belongs to this repository travels with the package ────────────────
step "build the tarball (release side; needs cargo)"
cargo build --quiet --release -p gmr-cli
mkdir -p "$bundle/bin"
cp "$root/target/release/gmr" "$bundle/bin/gmr"

# This repository's own probe store and its memories are bootstrap data, not
# product. Shipping either would be wrong.
[ -e "$bundle/probes" ] && fail "the tarball carries probes/ — that is this repo's bootstrap data"
[ -e "$bundle/memories" ] && fail "the tarball carries memories/ — that is this repo's own judgement"

step "install the way dist/install.sh does"
mkdir -p "$prefix/bin"
cp "$bundle/bin/gmr" "$prefix/bin/gmr"
gmr="$prefix/bin/gmr"

# ── A stranger's repository: no Rust, no Cargo.toml, nothing downloaded ──────
step "init leaves the criteria to the owner and copies nothing in"
mkdir -p "$repo/src"
printf 'export function createSession(id: string) { return { id }; }\n' > "$repo/src/auth.ts"
(cd "$repo" && git init -q . && git add -A \
    && git -c user.email=a@b -c user.name=t commit -qm init)
[ -f "$repo/Cargo.toml" ] && fail "the fixture repo must not have a Cargo.toml"

out=$("$gmr" --repo "$repo" init)
[ -f "$repo/.anchor/anchors.toml" ] \
    && fail "init wrote a declaration; criteria belong to the owner, not the tool" "$out"
[ -f "$repo/.anchor/.gitignore" ] || fail "init did not write .anchor/.gitignore" "$out"
[ -d "$repo/.anchor/probes" ] && [ -n "$(ls -A "$repo/.anchor/probes" 2>/dev/null)" ] \
    && fail "init copied probes in; the extractors are in the binary, nothing should land" "$out"

# ── User side: every promise, in every world, against every store ────────────
step "the promises, expanded over every world and every store"
python3 "$root/tools/acceptance.py" --binary "$gmr" "$@"

echo
echo "Accepted: a stranger's repo, no toolchain, nothing downloaded. An argument"
echo "          is traceable to a signal that exists, and when the signal moves"
echo "          the memory comes back — in the source and outside it."
echo "ACCEPTANCE COMPLETE steps=$steps"

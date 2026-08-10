#!/bin/sh
# The acceptance criterion, in one sentence:
#
#   A stranger, in a non-Rust repository that has nothing to do with GMR, with no
#   Rust toolchain, can tie one memory to a fact, and get that memory handed back
#   when the fact moves.
#
# This script runs the whole chain starting from a packaged bundle. It is
# **deliberately not part of gate.sh**: gate.sh only inspects the substrate and
# the batteries and never touches an anchor, because a red anchor is a signal for
# a person, not a build failure. What is asserted here are the anchors of a
# fixture repository -- that is test data, and asserting it is the job.
#
# The last line printed is a sentinel, and CI greps for it with the step count.
# That is not decoration: this file was once silently truncated mid-heredoc by an
# editing pass, `sh` treated the unterminated `<<'EOF'` as delimited by
# end-of-file, and the script went on exiting 0 having tested almost nothing for
# two days. `sh -n` does not catch that. The sentinel does, and tools/gate.py
# checks the sentinel and the heredoc pairing on every PR.
set -eu

root=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
bundle=$work/bundle
prefix=$work/prefix
repo=$work/repo

fail() {
    echo
    echo "Acceptance failed: $1"
    [ $# -gt 1 ] && { echo "--- actual output ---"; echo "$2"; }
    exit 1
}

steps=0
step() { steps=$((steps + 1)); echo; echo "── $1"; }

# ── Shipping side: one binary. The extractor chain is inside it, nothing else
#    travels with the package ────────────────────────────────────────────────
step "build the tarball (release side; needs cargo)"
cargo build --quiet --release -p coding-anchor
mkdir -p "$bundle/bin"
cp "$root/target/release/gmr" "$bundle/bin/gmr"

# This repository's own probe store is bootstrap data, not product. Shipping it
# would be wrong.
[ -e "$bundle/probes" ] && fail "tarball contains probes/ — this is the repo's bootstrap data"

step "install the way dist/install.sh does"
mkdir -p "$prefix/bin"
cp "$bundle/bin/gmr" "$prefix/bin/gmr"

gmr="$prefix/bin/gmr"

# ── User side: an ordinary TS repository. No Rust source, no Cargo.toml ──────
step "a stranger's TypeScript repo"
mkdir -p "$repo/src"
cat > "$repo/src/auth.ts" <<'EOF'
export function createSession(userId: string, ttl: number): Session {
  return { userId, ttl };
}
export const verify = (token: string) => token.length > 0;
EOF
(cd "$repo" && git init -q . && git add -A \
    && git -c user.email=a@b -c user.name=t commit -qm init)

[ -f "$repo/Cargo.toml" ] && fail "the fixture repo must not have a Cargo.toml"

step "init"
out=$("$gmr" --repo "$repo" init)
echo "$out" | grep -q "ast-map" || fail "init did not report the built-in probes" "$out"
echo "$out" | grep -q "\.ts" || fail "init did not recognise TypeScript" "$out"
[ -f "$repo/.anchor/anchors.toml" ] && fail "init wrote an anchor declaration; criteria belong to the owner, not the tool"
[ -f "$repo/.anchor/.gitignore" ] || fail "init did not write .anchor/.gitignore"
[ -d "$repo/.anchor/probes" ] && [ -n "$(ls -A "$repo/.anchor/probes" 2>/dev/null)" ] \
    && fail "init copied probes into the repository; the extractors are in the binary, nothing should land"

# The identity has to be earned, and it has to be the kind that compares across
# machines.
out=$("$gmr" --repo "$repo" probes list --json)
echo "$out" | grep -q '"kind":"builtin"' || fail "the built-in probes were not listed" "$out"
echo "$out" | python3 -c '
import json, sys
probes = json.load(sys.stdin)["probes"]
bad = [p["probe"] for p in probes if len(p.get("version") or "") != 64]
sys.exit("not an earned hash: " + ", ".join(bad) if bad else 0)
' || fail "a probe version is not an earned hash" "$out"

# ── One line of frontmatter. The user writes no anchors.toml and no rule table ─
step "one line of frontmatter"
printf -- '---\nabout: src/auth.ts#createSession\n---\n\n# A session is only created inside the service boundary\n' \
    > "$repo/memories/auth.md"
(cd "$repo" && git add -A && git -c user.email=a@b -c user.name=t commit -qm note)

out=$("$gmr" --repo "$repo" sync)
echo "$out" | grep -q "1 anchors opened" || fail "the note did not open an anchor" "$out"
echo "$out" | grep -q "memories/auth.md" || fail "the note was not bound" "$out"

# A second run must write nothing: the binding table only grows, never changes.
out=$("$gmr" --repo "$repo" sync)
echo "$out" | grep -q "memories/auth.md" && fail "sync is not idempotent, it appended the binding again" "$out"

step "observe: the world has not moved"
set +e
out=$("$gmr" --repo "$repo" observe); code=$?
set -e
[ "$code" -eq 0 ] || fail "the exit code should be 0 when the world has not moved, got $code" "$out"

# ── The fact moves ──────────────────────────────────────────────────────────
step "the world moves"
cat > "$repo/src/auth.ts" <<'EOF'
export function createSession(userId: string, ttl: number, scope: string): Session {
  return { userId, ttl, scope };
}
export const verify = (token: string) => token.length > 0;
EOF

set +e
out=$("$gmr" --repo "$repo" observe); code=$?
set -e
[ "$code" -eq 1 ] || fail "the exit code should be 1 when the world moved, got $code" "$out"
echo "$out" | grep -q "moved" || fail "did not report moved" "$out"
# This line is the acceptance criterion itself: the fact moved, the note came back.
echo "$out" | grep -q "memories/auth.md" \
    || fail "the anchor moved but the note bound to it was not handed back — this is the whole value of the product" "$out"

step "pass --json: the shape an agent loop reads"
"$gmr" --repo "$repo" requeue 'src/auth.ts#createSession' >/dev/null
cat > "$repo/src/auth.ts" <<'EOF'
export function createSession(userId: string): Session { return { userId }; }
EOF
set +e
out=$("$gmr" --repo "$repo" pass --json); code=$?
set -e
[ "$code" -eq 1 ] || fail "pass should exit 1 when an anchor moved, got $code" "$out"
echo "$out" | grep -q '"memories":\["memories/auth.md"\]' \
    || fail "pass --json did not hand the note back" "$out"

# ── A probe the user wrote: a fact that is not in the code ───────────────────
step "a probe the user wrote themselves"
mkdir -p "$repo/scripts"
cat > "$repo/scripts/deploy.sh" <<'EOF'
#!/bin/sh
printf '{"sha":"a1b2c3d"}\n'
EOF
chmod +x "$repo/scripts/deploy.sh"
cat > "$repo/.anchor/probes.toml" <<'EOF'
[script.deploy-sha]
run = "scripts/deploy.sh"
obs = { schema = "gmr.probe-deploy.v1", at = [], facts = ["sha"] }
EOF
cat > "$repo/.anchor/anchors.toml" <<'EOF'
[[anchor]]
key = "deploy::staging"
probe = "deploy-sha"
position = { env = "staging" }
rules = [
  'not exists(state.sha) => { position: state.position, sha: obs.sha, status: "captured" }',
  'obs.sha != state.sha => { position: state.position, sha: obs.sha, was: state.sha, status: "redeployed" }',
]
EOF
printf -- '---\nanchors:\n  - deploy::staging\n---\n\n# which commit is running on staging\n' \
    > "$repo/memories/deploy.md"
(cd "$repo" && git add -A && git -c user.email=a@b -c user.name=t commit -qm deploy)

out=$("$gmr" --repo "$repo" sync)
echo "$out" | grep -q "1 anchors opened" || fail "the script probe's anchor did not open" "$out"
"$gmr" --repo "$repo" observe >/dev/null

sed -i.bak 's/a1b2c3d/9f8e7d6/' "$repo/scripts/deploy.sh"
set +e
out=$("$gmr" --repo "$repo" observe); code=$?
set -e
[ "$code" -eq 1 ] || fail "the deploy moved to another commit, the exit code should be 1, got $code" "$out"
echo "$out" | grep -q "memories/deploy.md" \
    || fail "anchored a fact that is invisible in the source, but the note did not come back" "$out"

# ── The front door: one coordinate in, one vector out ────────────────────────
step "the front door: anchor / status / check / accept"
cat > "$repo/src/session.ts" <<'EOF'
export function rotate(id: string): string { return id; }
EOF
(cd "$repo" && git add -A && git -c user.email=a@b -c user.name=t commit -qm session)

key='src/session.ts#rotate'
out=$("$gmr" --repo "$repo" anchor "$key" -m 'rotation must complete before the write')
echo "$out" | grep -q 'missing · kind · sig · surface · logic · place' \
    || fail "anchor did not derive contract's six axes from the coordinate" "$out"
echo "$out" | grep -q 'memories/session-rotate.md' || fail "anchor did not write the note" "$out"

out=$("$gmr" --repo "$repo" status "$key")
echo "$out" | grep -q 'contract' || fail "status did not recognise the shape" "$out"
echo "$out" | grep -q 'memories/session-rotate.md' || fail "status did not list the memory" "$out"

set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 0 ] || fail "check should be 0 when the world has not moved, got $code" "$out"

# A few extra lines above is not a move. Position measures "who do you sit after",
# not an absolute line number -- otherwise one import at the top of a file would
# light up every anchor below it, and not one of them moved.
cat > "$repo/src/session.ts" <<'EOF'
// a
// b
export function rotate(id: string): string { return id; }
EOF
set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 0 ] || fail "two comment lines above is not a move, check should not fire, got $code" "$out"

# Really changing place is a move: a definition appeared before it. Whether that
# is reasonable is the author's call; the tool's job is to put it on the table.
cat > "$repo/src/session.ts" <<'EOF'
export function issue(id: string): string { return id; }
export function rotate(id: string): string { return id; }
EOF
set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 1 ] || fail "a definition appeared before it, check should be 1, got $code" "$out"
echo "$out" | grep -q 'memories/session-rotate.md' \
    || fail "place moved but the memory was not handed back" "$out"

# Say so yourself if you want quiet. watch is per-note, not a criterion, and
# changing it needs no sealed rationale -- while the place bits already
# accumulated stay visible in status.
note="$repo/memories/session-rotate.md"
printf '%s\n' '---' "about: $key" 'watch: [sig, logic]' '---' '' 'rotation must complete before the write' > "$note"
cat > "$repo/src/session.ts" <<'EOF'
export function issue(id: string): string { return id; }
export function later(id: string): string { return id; }
export function rotate(id: string): string { return id; }
EOF
set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 0 ] || fail "the note only subscribed to sig/logic, a place move should not fire, got $code" "$out"
out=$("$gmr" --repo "$repo" status "$key")
echo "$out" | grep -q 'place 1' || fail "not handing the memory over is not the same as not accounting for it; the place bit should still be there" "$out"
printf '%s\n' '---' "about: $key" '---' '' 'rotation must complete before the write' > "$note"

# Signature, implementation, public surface and place all move together. All four
# have to land -- an ordered rule table would report only the first and swallow
# the other three.
cat > "$repo/src/session.ts" <<'EOF'
export function issue(id: string): string { return id; }
export function later(id: string): string { return id; }
export function gate(id: string): string { return id; }
/** @deprecated */
export function rotate(id: string, now: number): string {
  const next = id + now;
  return next;
}
EOF
set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 1 ] || fail "a subscribed axis moved, check should be 1, got $code" "$out"
echo "$out" | grep -q 'memories/session-rotate.md' || fail "check did not hand the memory back" "$out"

out=$("$gmr" --repo "$repo" status "$key")
echo "$out" | grep -qE 'sig 1' || fail "the signature changed but the sig bit was not set" "$out"
echo "$out" | grep -qE 'logic 1' || fail "the implementation changed but the logic bit was not set — this is the one the old table swallowed" "$out"
echo "$out" | grep -qE 'place 1' || fail "place changed but the place bit was not set" "$out"

out=$("$gmr" --repo "$repo" accept "$key" --why 'one more now parameter does not affect "rotation precedes the write"')
echo "$out" | grep -q 'rationale' || fail "accept did not seal the rationale" "$out"

out=$("$gmr" --repo "$repo" status "$key")
echo "$out" | grep -q 'sig 0' || fail "the sig bit should be zero after accept" "$out"
set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 0 ] || fail "check should be 0 after accept, got $code" "$out"

# When a letter is mistyped, what it says has to be "there is no such anchor",
# and it has to point at the closest one -- not "the lease is held by someone
# else", which reports our failure as a state of the world.
step "a mistyped key speaks plainly"
set +e
out=$("$gmr" --repo "$repo" check 'src/session.ts#rotare' 2>&1); code=$?
set -e
[ "$code" -ne 0 ] || fail "an anchor that does not exist should not succeed" "$out"
echo "$out" | grep -q 'no anchor matches' || fail "did not make clear the key was wrong" "$out"
echo "$out" | grep -q "$key" || fail "did not point at the closest key" "$out"
out=$("$gmr" --repo "$repo" status 'src/session.ts')
echo "$out" | grep -q "$key" || fail "a read-only verb should expand a file prefix" "$out"

# What accept pins has to be the fact as of now, not the reading the last
# observation left in state. A person checks, sees red, reverts the change, then
# accepts -- if accept takes that stale reading it pins the broken one as the
# baseline, and good code then reports "changed" forever.
step "accept takes a fresh look, it does not pin the previous reading"
sed -i.bak 's/const next = id + now;/const next = id;/' "$repo/src/session.ts" && rm -f "$repo/src/session.ts.bak"
set +e
"$gmr" --repo "$repo" check "$key" >/dev/null; code=$?
set -e
[ "$code" -eq 1 ] || fail "after breaking it check should be 1, got $code"
sed -i.bak 's/const next = id;/const next = id + now;/' "$repo/src/session.ts" && rm -f "$repo/src/session.ts.bak"
out=$("$gmr" --repo "$repo" accept "$key" --why 'looked at it, the change was reverted')
set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 0 ] || fail "after reverting and accepting, check should be 0; accept pinned a stale reading" "$out"

# ── Scale: a repository has to be able to open an anchor at all ──────────────
# The fixture above is one file, and one file hides the failure this guards. A
# cache that rewrote the whole file once per file walked was quadratic in the
# size of the repository, so on a real one it spent the entire probe budget
# before answering anything. The visible symptom was not "slow": it was that
# every coordinate, file-level and symbol-level alike, timed out, and an anchor
# could never be opened in the first place.
#
# That is why the assertion here is that sync opens the anchor, not a stopwatch.
# It is binary and it does not care how fast the runner is. Measured on this
# fixture, 1600 files:
#
#     before   sync fails: "probe did not return within 30s", 0 anchors
#     after    sync 0.51 s, cold check 0.53 s
#
# The wall-clock bar underneath it is a coarse backstop, deliberately far above
# the real cost so a slow runner cannot flake it. There is no warm bar yet:
# today a warm run still walks and hashes every file, so warm is the same either
# way. It becomes worth asserting when the index lands and a warm query stops
# touching the tree at all.
step "scale: a repository large enough that the old cost was fatal"
scale=$work/scale
mkdir -p "$scale/src"
i=1
while [ "$i" -le 1600 ]; do
    j=1
    while [ "$j" -le 20 ]; do
        printf 'export function build_%d_%d(x: number): number { return x + %d; }\n' "$i" "$j" "$j"
        j=$((j + 1))
    done > "$scale/src/f$i.ts"
    i=$((i + 1))
done
(cd "$scale" && git init -q . && git add -A \
    && git -c user.email=a@b -c user.name=t commit -qm init)
"$gmr" --repo "$scale" init >/dev/null
printf -- '---\nabout: src/f1.ts#build_1_1\n---\n\n# the first one\n' > "$scale/memories/n.md"
(cd "$scale" && git add -A && git -c user.email=a@b -c user.name=t commit -qm note)

set +e
out=$("$gmr" --repo "$scale" sync 2>&1); code=$?
set -e
[ "$code" -eq 0 ] || fail "sync could not open an anchor on a 1600-file repository" "$out"
echo "$out" | grep -q "1 anchors opened" \
    || fail "1600 files and the anchor never opened — the scan cost is not proportional to the repository" "$out"

rm -f "$scale/.anchor/state/extract-cache.json"
start=$(date +%s)
"$gmr" --repo "$scale" check >/dev/null
cold=$(( $(date +%s) - start ))
[ "$cold" -lt 15 ] || fail "a cold scan of 1600 files took ${cold}s; the per-file cost is not proportional any more"

[ -s "$scale/.anchor/state/extract-cache.json" ] \
    || fail "the scan left no cache behind, so every later run pays the full price again"

echo
echo "Accepted: a stranger's repo, no toolchain, no downloaded probes. Memory and"
echo "          fact are tied together — in the source and outside it — and when the"
echo "          fact moves the memory comes back."
echo "ACCEPTANCE COMPLETE steps=$steps"

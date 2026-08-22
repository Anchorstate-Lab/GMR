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
echo "$out" | grep -q -- "+ auth" || fail "the note was not bound" "$out"

# A second run must write nothing: the binding table only grows, never changes.
out=$("$gmr" --repo "$repo" sync)
echo "$out" | grep -q -- "+ auth" && fail "sync is not idempotent, it appended the binding again" "$out"

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
echo "$out" | grep -q -- "→ auth" \
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
echo "$out" | grep -q '"memories":\["git:memories/auth.md"\]' \
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
echo "$out" | grep -q -- "→ deploy" \
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
echo "$out" | grep -q -- '→ session-rotate' || fail "status did not list the memory" "$out"
echo "$out" | grep -q 'memories/session-rotate.md' \
    && fail "status spelled a note as a path while every other verb spells it as a name" "$out"

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
echo "$out" | grep -q -- '→ session-rotate' \
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
echo "$out" | grep -q -- '→ session-rotate' || fail "check did not hand the memory back" "$out"

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

# Deleting the note is how an anchor stops being supervised without anybody
# closing it. The anchor keeps its journal, keeps being observed, keeps
# answering -- and nothing compares its criteria against anything, because
# there is no declaration left to compare against. That used to be a `continue`
# nobody could see.
step "an anchor no note declares any more says so"
mv "$repo/memories/session-rotate.md" "$repo/session-rotate.md.away"
set +e
out=$("$gmr" --repo "$repo" check 2>&1); code=$?
set -e
[ "$code" -ne 0 ] || fail "an anchor nothing declares should not report a clean run" "$out"
echo "$out" | grep -q 'supervised by no note' \
    || fail "the orphaned anchor was skipped in silence" "$out"
echo "$out" | grep -q "$key" || fail "did not name which anchor lost its note" "$out"

set +e
out=$("$gmr" --repo "$repo" doctor 2>&1); code=$?
set -e
[ "$code" -ne 0 ] || fail "doctor called a repository healthy while an anchor had no note" "$out"
echo "$out" | grep -q 'undeclared' || fail "doctor did not report the anchor as undeclared" "$out"

mv "$repo/session-rotate.md.away" "$repo/memories/session-rotate.md"
"$gmr" --repo "$repo" check "$key" >/dev/null \
    || fail "putting the note back did not restore the anchor to a clean check"

# A note that fails to route is not the same fact as no note at all: the note
# is right there naming the anchor, so this is `unreadable` (fix the
# coordinate), not `undeclared` (write the note again) -- two different
# causes with two different remedies. doctor computed "undeclared" as a
# second walk that did not know about blocked faults, so it folded this case
# into the wrong one; check, which does know, did not.
step "a note that fails to route is unreadable, not undeclared"
printf '%s\n' '---' "about: $key" 'shape: not-a-real-shape' '---' '' \
    'rotation must complete before the write' > "$note"
set +e
out=$("$gmr" --repo "$repo" check "$key" 2>&1); code=$?
set -e
[ "$code" -ne 0 ] || fail "an unrouted coordinate should not report a clean check" "$out"
echo "$out" | grep -q 'could not read' \
    || fail "an unrouted coordinate should be reported as unreadable" "$out"
echo "$out" | grep -q 'supervised by no note' \
    && fail "an unrouted coordinate is not the same as no note at all" "$out"

set +e
out=$("$gmr" --repo "$repo" doctor 2>&1); code=$?
set -e
[ "$code" -ne 0 ] || fail "doctor called a repository healthy while a note fails to route" "$out"
echo "$out" | grep -q "undeclared.*$key" \
    && fail "doctor mistook an unrouted note for a deleted one" "$out"

printf '%s\n' '---' "about: $key" '---' '' 'rotation must complete before the write' > "$note"
"$gmr" --repo "$repo" check "$key" >/dev/null \
    || fail "fixing the shape did not restore the anchor to a clean check"

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

rm -f "$scale/.anchor/state/survey-index.sqlite" "$scale/.anchor/state/survey-index.sqlite-wal" \
    "$scale/.anchor/state/survey-index.sqlite-shm"
start=$(date +%s)
"$gmr" --repo "$scale" check >/dev/null
cold=$(( $(date +%s) - start ))
[ "$cold" -lt 15 ] || fail "a cold scan of 1600 files took ${cold}s; the per-file cost is not proportional any more"

[ -s "$scale/.anchor/state/survey-index.sqlite" ] \
    || fail "the scan left no index behind, so every later run pays the full price again"

# ── A budget that runs out has to be *loud*. This is the failure the whole
#    design points at: an anchor nobody could look at must come back as a
#    refusal a person sees, and must never arrive as a quiet answer that drives
#    a transition. A budget may produce no answer; it may not produce a shorter
#    one.
#
#    The wall-clock version of this step -- "the probe sleeps eight seconds
#    against a 300ms budget, assert the process is gone in two" -- was measured
#    and deliberately not written, because there is no longer a signal to
#    assert. A cold scan of the 1600 files above costs 0.53s here, and a single
#    5.6 MB file (the case the per-file checkpoint cannot interrupt) costs
#    1.07s; showing an 8:0.3 ratio needs tens of thousands of files. The only
#    probe kind that can be made slow to order is a script probe, and its
#    cancellation runs through kill_on_drop, never through the blocking-thread
#    path the incident was actually about. A timing assertion built anyway would
#    have a margin thinner than the runner's noise, go flaky, and be deleted --
#    which is how green idling starts. So what is pinned here is the part that
#    survived the extractors getting fast: the refusal is typed, it names its
#    own width, and it does not become state.
step "a budget that runs out refuses out loud, and refusing is not an answer"
rm -f "$scale/.anchor/state/survey-index.sqlite" "$scale/.anchor/state/survey-index.sqlite-wal" \
    "$scale/.anchor/state/survey-index.sqlite-shm"
set +e
out=$("$gmr" --repo "$scale" --probe-budget-ms 1 check 2>&1); code=$?
set -e

[ "$code" -ne 0 ] || fail "a probe that never got to look exited 0 — silence was read as agreement" "$out"
echo "$out" | grep -q "could not be looked at" \
    || fail "a spent budget was not reported as an anchor nobody looked at" "$out"
echo "$out" | grep -q "TimedOut" \
    || fail "the budget ran out and the reading did not say which failure it was" "$out"
echo "$out" | grep -q "silence is not evidence" \
    || fail "the refusal did not say why silence proves nothing" "$out"
echo "$out" | grep -q "nothing moved" \
    && fail "a spent budget was reported as a settled anchor — this is the silent lie" "$out"

# The same anchor, with time to look, settles. A refusal that turned into state
# would still be here, and no later run would clear it.
"$gmr" --repo "$scale" check >/dev/null \
    || fail "the anchor did not settle once it had time to look — a spent budget became state"

# ── Who can fix it decides the exit code. Every state below is reachable with
#    the claude-code provider alone, because its store is a directory this
#    script owns: point it somewhere, and the store is there, empty, or gone.
#    Without these, the rule that `unreachable` never turns a build red is a
#    sentence in a doc that no run disagrees with.
step "the exit code is decided by who can fix it"
mem=$work/claude-memory
mkdir -p "$mem"
printf 'binding through a store this script owns\n' > "$mem/owned.md"
export GMR_CLAUDE_MEMORY_DIR="$mem"

"$gmr" --repo "$repo" bind owned.md --provider claude-code \
    --anchors 'src/auth.ts#createSession' >/dev/null \
    || fail "could not bind through the claude-code provider"

set +e
out=$("$gmr" --repo "$repo" doctor --json); code=$?
set -e
[ "$code" -eq 0 ] || fail "a healthy binding through a second store made doctor red" "$out"

# The record is deleted, the store is still there: the world's answer.
rm "$mem/owned.md"
set +e
out=$("$gmr" --repo "$repo" doctor --json); code=$?
set -e
[ "$code" -eq 1 ] || fail "a record the store says is gone did not turn doctor red, and unbinding it is exactly the thing the owner can do" "$out"
echo "$out" | grep -q '"gone":\["claude-code:owned.md"\]' || fail "the dead reference was not reported as gone" "$out"

# The store itself is gone: our failure, not the world's answer. Same repository,
# same binding — only the reachability differs, and the exit code must not.
printf 'back again\n' > "$mem/owned.md"
"$gmr" --repo "$repo" reaffirm owned.md --provider claude-code >/dev/null \
    || fail "could not re-stamp the binding after the record came back"
export GMR_CLAUDE_MEMORY_DIR="$work/no-such-store"
set +e
reachable=$("$gmr" --repo "$repo" check >/dev/null 2>&1; echo $?)
out=$("$gmr" --repo "$repo" doctor --json); code=$?
set -e
echo "$out" | grep -q '"unreachable":\["claude-code:owned.md"\]' \
    || fail "a store that is not there was not reported as unreachable — silence is how this used to look" "$out"
echo "$out" | grep -q '"gone":\[\]' \
    || fail "a store that would not answer was read as the record being gone; that sends the reader to delete a binding that is fine" "$out"
[ "$code" -eq 0 ] || fail "somebody else's store being unreachable turned doctor red; nobody holding this repository can act on that" "$out"

export GMR_CLAUDE_MEMORY_DIR="$mem"
set +e
"$gmr" --repo "$repo" check >/dev/null 2>&1; back=$?
set -e
[ "$reachable" -eq "$back" ] \
    || fail "check exited $reachable with the store unreachable and $back with it reachable — a store nobody here owns must not move the exit code"

# The provider cannot even register: with no $HOME and no override there is no
# directory to name. A binding through a store this binary has no provider for
# is the owner's to fix -- enable a feature, set credentials, rebind -- so red.
set +e
out=$(env -u HOME -u GMR_CLAUDE_MEMORY_DIR "$gmr" --repo "$repo" doctor --json); code=$?
set -e
echo "$out" | grep -q '"no_provider":\["claude-code:owned.md"\]' \
    || fail "a binding through a store with no provider in this binary was not reported as such; unreachable would say somebody else's service is down, and this is a build or a config" "$out"
[ "$code" -eq 1 ] || fail "a binding through a store this binary cannot name did not turn doctor red" "$out"

step "a listing is what a store will show, not a roster of what exists"
out=$("$gmr" --repo "$repo" memories --json)
echo "$out" | grep -q '"reference":"git:memories/auth.md"' \
    || fail "the note this run bound was not in its store's listing" "$out"
echo "$out" | grep -q '"anchors":\["src/auth.ts#createSession"\]' \
    || fail "the listing did not say which anchors a bound record is about" "$out"
"$gmr" --repo "$repo" memories --provider claude-code >/dev/null 2>&1 \
    && fail "a store with no way to list what it holds answered a listing anyway"

# ── SKILL.md tells an agent to hand an address straight back to the verbs. That
#    sentence is a promise about two things that are edited in different files,
#    and nothing but a round trip can tell whether they still agree. Without
#    this, `bind` answered a valid address with "`git` has no record
#    `git:memories/auth.md`" and `cobound` answered about an address nobody
#    ever wrote — exit 0, no error anywhere.
step "an address this CLI prints is an address this CLI takes"
addr=$("$gmr" --repo "$repo" memories --json \
    | tr ',' '\n' | grep -o '"git:memories/auth.md"' | head -1 | tr -d '"')
[ "$addr" = "git:memories/auth.md" ] || fail "could not read an address out of --json" "$addr"

"$gmr" --repo "$repo" reaffirm "$addr" >/dev/null \
    || fail "a verb refused the address its own --json had just printed"
"$gmr" --repo "$repo" cobound "$addr" >/dev/null \
    || fail "cobound refused the address its own --json had just printed"

# The half git alone cannot prove: a store whose prefix is not `git`.
export GMR_CLAUDE_MEMORY_DIR="$mem"
"$gmr" --repo "$repo" reaffirm 'claude-code:owned.md' >/dev/null \
    || fail "an address naming a second store did not resolve to that store"

set +e
out=$("$gmr" --repo "$repo" bind "$addr" --provider claude-code --anchors "$key" 2>&1); code=$?
set -e
[ "$code" -ne 0 ] \
    || fail "an address saying git and a --provider saying claude-code were reconciled by guessing; the binding table only ever grows" "$out"

step "an installed SKILL.md older than the binary is the owner's to fix"
printf 'stale\n' >> "$repo/.claude/skills/gmr/SKILL.md"
set +e
out=$("$gmr" --repo "$repo" doctor --json); code=$?
set -e
[ "$code" -eq 1 ] || fail "an installed skill doc this build no longer honours did not turn doctor red — agents read it and cannot tell" "$out"
echo "$out" | grep -q '"skill_stale":\[' || fail "the stale skill doc was not named" "$out"
rm "$repo/.claude/skills/gmr/SKILL.md"
"$gmr" --repo "$repo" init >/dev/null

step "a record that is gone can still be let go of"
"$gmr" --repo "$repo" bind 'git:memories/auth.md' --anchors "$key" >/dev/null \
    || fail "could not bind the note back before deleting it"
rm "$repo/memories/auth.md"
(cd "$repo" && git add -A && git -c user.email=a@b -c user.name=t commit -qm "delete the note")

set +e
out=$("$gmr" --repo "$repo" doctor --json)
set -e
echo "$out" | grep -q '"gone":\["git:memories/auth.md"\]' \
    || fail "a deleted record was not reported as gone" "$out"

set +e
out=$("$gmr" --repo "$repo" bind 'git:memories/auth.md' --detach 2>&1); code=$?
set -e
[ "$code" -eq 0 ] || fail "doctor says to restore the record or detach the binding, and detach refused: the one state that needs an unbind is the one where the record cannot be fetched" "$out"

set +e
out=$("$gmr" --repo "$repo" doctor --json)
set -e
echo "$out" | grep -q '"gone":\[\]' || fail "detaching left the record still reported as gone" "$out"
echo "$out" | grep -q '"unsupervised":\[\]' \
    || fail "detaching left the record reported as unsupervised" "$out"

# ── The claim D2 makes is that a store can be taught to this binary without
#    teaching it to the compiler. Nothing but a run against the shipped
#    tarball can hold that claim honest: a recipe, two shell scripts, and
#    every state a compiled provider reaches — listed, bound, rewritten, gone.
step "a store declared in a recipe, with no Rust anywhere"
mkdir -p "$repo/scripts" "$work/desk"
cat > "$repo/scripts/desk-fetch.sh" <<'SH'
#!/bin/sh
id=$(printf '%s' "$GMR_POSITION" | sed 's/.*"id":"\([^"]*\)".*/\1/')
file="$DESK/$id"
[ -f "$file" ] || { printf 'null'; exit 0; }
printf '{"text":"%s"}' "$(sed 's/"/\\"/g' "$file" | tr -d '\n')"
SH
cat > "$repo/scripts/desk-list.sh" <<'SH'
#!/bin/sh
printf '{"records":['
first=1
for f in "$DESK"/*; do
  [ -f "$f" ] || continue
  [ $first -eq 1 ] || printf ','
  first=0
  printf '{"id":"%s","text":"%s"}' "$(basename "$f")" "$(sed 's/"/\\"/g' "$f" | tr -d '\n')"
done
printf ']}'
SH
chmod +x "$repo/scripts/desk-fetch.sh" "$repo/scripts/desk-list.sh"
cat > "$repo/.anchor/providers.toml" <<'TOML'
[provider.desk]
fetch = "scripts/desk-fetch.sh"
list = "scripts/desk-list.sh"
TOML
export DESK="$work/desk"
printf 'the 30 minutes is the CDN cache window, not a security choice\n' > "$DESK/why-30.md"

out=$("$gmr" --repo "$repo" memories --provider desk --json)
echo "$out" | grep -q '"reference":"desk:why-30.md"' \
    || fail "a store declared in a recipe did not list what it holds" "$out"

"$gmr" --repo "$repo" bind 'desk:why-30.md' --anchors "$key" >/dev/null \
    || fail "could not bind through a provider nobody compiled in" "$out"
out=$("$gmr" --repo "$repo" read "$key")
echo "$out" | grep -q 'desk:why-30.md' || fail "the bound record was not delivered on its anchor" "$out"
echo "$out" | grep -qE 'never verified|rewritten|gone' \
    && fail "a record the recipe can read did not ground as current" "$out"

# The version has to move with the bytes, or nothing downstream can tell a
# memory was rewritten -- the quietest way this system can fail.
printf 'it is 45 minutes now, and for a different reason\n' > "$DESK/why-30.md"
out=$("$gmr" --repo "$repo" read "$key")
echo "$out" | grep -q 'rewritten since binding' \
    || fail "a rewritten record read as current through a declared provider" "$out"

# And the world's answer must still be distinguishable from our failure.
rm "$DESK/why-30.md"
set +e
out=$("$gmr" --repo "$repo" doctor --json)
set -e
echo "$out" | grep -q '"gone":\["desk:why-30.md"\]' \
    || fail "a record the script says is not there was not reported as gone" "$out"

"$gmr" --repo "$repo" bind 'desk:why-30.md' --detach >/dev/null \
    || fail "could not let go of a record held through a declared provider"
rm "$repo/.anchor/providers.toml"

# ── The main road, agent side: it writes a memory into its own store and says
#    what that memory is about in the same breath. A store has not indexed the
#    record yet at that moment, which is exactly when the link is most accurate
#    and least provable — so the assertion lands unverified rather than refused,
#    and says out loud that only its own writer stands behind it.
step "an agent binds what it just wrote, before any store can answer for it"
export GMR_CLAUDE_MEMORY_DIR="$mem"
rm -f "$mem/fresh.md"

set +e
out=$("$gmr" --repo "$repo" attest 'claude-code:fresh.md' --anchors "$key" --json); code=$?
set -e
[ "$code" -eq 0 ] \
    || fail "a record the store could not answer for was refused, and that is the one link nothing else can reconstruct" "$out"
echo "$out" | grep -q '"source":"self_attested"' \
    || fail "the agent's own say-so was recorded as something else" "$out"
echo "$out" | grep -q '"vouched":false' \
    || fail "an agent vouching for its own record was reported as independently established" "$out"
echo "$out" | grep -q '"version":null' \
    || fail "a version was claimed for a record no store had answered for" "$out"

# The store catches up. Nothing on the read path settles a baseline, so the
# record is readable and still never compared.
printf 'written by the agent, indexed a moment later\n' > "$mem/fresh.md"
out=$("$gmr" --repo "$repo" read "$key")
echo "$out" | grep -q 'never verified' \
    || fail "a record nothing has ever compared read as though a baseline stood behind it" "$out"
echo "$out" | grep -q 'only the writer of this record' \
    || fail "nothing said this link rests on the agent's own say-so; a reader cannot weigh what is not shown" "$out"

# The same verb again is how an agent stamps the baseline it could not take the
# first time. `reaffirm` would record a person's judgement, and no agent gets to
# launder its own say-so into one by running a second command.
"$gmr" --repo "$repo" attest 'claude-code:fresh.md' --anchors "$key" >/dev/null \
    || fail "re-attesting an already bound record was refused"
out=$("$gmr" --repo "$repo" read "$key")
echo "$out" | grep -q 'never verified' \
    && fail "an assertion that did reach the store left the record still unverified" "$out"
echo "$out" | grep -q 'only the writer of this record' \
    || fail "a second act by the same agent was reported as though somebody else had vouched for the link" "$out"

# And a later assertion that reached nothing does not undo the baseline above.
rm "$mem/fresh.md"
"$gmr" --repo "$repo" attest 'claude-code:fresh.md' --anchors "$key" >/dev/null \
    || fail "attesting a record the store no longer answers for was refused"
printf 'written by the agent, indexed a moment later\n' > "$mem/fresh.md"
out=$("$gmr" --repo "$repo" read "$key")
echo "$out" | grep -q 'never verified' \
    && fail "an assertion that compared nothing threw away a reading somebody really took" "$out"

echo
echo "Accepted: a stranger's repo, no toolchain, no downloaded probes. Memory and"
echo "          fact are tied together — in the source and outside it — and when the"
echo "          fact moves the memory comes back."
echo "ACCEPTANCE COMPLETE steps=$steps"

#!/usr/bin/env bash
#
# GMR — demo recording script
# ---------------------------------------------------------------
# Everything shown here is real: a real function, a real memory
# someone already wrote about it, and the real commit that later
# changed the code under it — replayed in an isolated git worktree
# so the recording can never touch this repo's own anchor state.
#
# Press record, run this script, stop record. That's the whole video.
#
# USAGE
#   ./demo.sh            record a take
#   ./demo.sh --keep     leave the worktree in place afterward, for inspection
#
# Every take starts from a fresh throwaway worktree and removes it
# on exit (even on failure) — there is no separate --reset step and
# nothing to clean up by hand.
#
# TIP: make your terminal font BIG (16pt+) and the window ~100x30
#      before recording. Reviewers may watch this on a phone.
# ---------------------------------------------------------------

set -euo pipefail

# ============ CONFIG ============

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"   # the repo this script lives in

# The real coordinate, the real commit that changed it, and the real
# memory already sitting in this repo about it. See:
#   memories/survey-cache-scope.md
#   git show e7aa8ef
COORD="batteries/survey/src/cache.rs#visit_cached"
FILE="batteries/survey/src/cache.rs"
BEFORE_REV="2439389"   # the commit right before the fix
AFTER_REV="HEAD"       # this repo's current (fixed) code

# Pacing. Raise these if it feels rushed on playback.
TYPE_SPEED=0.035     # seconds per character while "typing"
READ_PAUSE=2.0       # pause after a caption, so viewers can read
BEAT=1.0             # short pause between steps

# ========================================================

DIM=$'\033[2m'; BOLD=$'\033[1m'; RED=$'\033[31m'; GREEN=$'\033[32m'; RESET=$'\033[0m'
PROMPT="${BOLD}\$${RESET} "

# --- preflight ---
command -v gmr >/dev/null || { echo "gmr not on PATH"; exit 1; }
git -C "$REPO" rev-parse --show-toplevel >/dev/null || { echo "REPO is not a git repo: $REPO"; exit 1; }
git -C "$REPO" cat-file -e "$BEFORE_REV" || { echo "commit not found: $BEFORE_REV"; exit 1; }

KEEP=0
[[ "${1:-}" == "--keep" ]] && KEEP=1

# --- isolated worktree: every take is disposable, the real repo is never touched ---
WT="$(mktemp -d "${TMPDIR:-/tmp}/gmr-demo.XXXXXX")"
WT="$(cd "$WT" && pwd -P)"   # canonicalize: on macOS /tmp is a symlink to /private/tmp
cleanup() {
  if [[ "$KEEP" == "1" ]]; then
    printf "\n%b\n" "${DIM}(kept: $WT)${RESET}"
  else
    git -C "$REPO" worktree remove --force "$WT" >/dev/null 2>&1 || rm -rf "$WT"
  fi
}
trap cleanup EXIT

git -C "$REPO" worktree add -q "$WT" HEAD

# every step below runs with an explicit `cd` guarded by this check —
# a failed cd must never silently fall through to the real repo
in_worktree() {
  [[ "$(pwd)" == "$WT" ]] || { echo "REFUSING: not inside the demo worktree"; exit 1; }
}

# --- helpers ---
type_line() {                      # simulate typing at a prompt
  printf "%b" "$PROMPT"
  local s="$1"
  for (( i=0; i<${#s}; i++ )); do
    printf "%s" "${s:$i:1}"
    sleep "$TYPE_SPEED"
  done
  printf "\n"
}

say() {                            # a caption — printed instantly, not typed
  printf "\n%b\n" "${DIM}# $1${RESET}"
  sleep "$READ_PAUSE"
}

run() {                            # type a command, then actually run it, in the worktree
  cd "$WT"; in_worktree
  type_line "$1"
  sleep 0.3
  eval "$1"
  sleep "$BEAT"
}

clear 2>/dev/null || printf '\033[2J\033[H'
sleep 1

# ================= SCENE 1 — a real memory that already exists =================
say "Not a demo fixture. A real function, and a real judgment someone already wrote about it."
run "cat memories/survey-cache-scope.md"

say "Rewinding the code to the commit before that judgment's baseline — the state it was written against."
cd "$WT"; in_worktree
git show "$BEFORE_REV:$FILE" > "$FILE"
gmr anchor >/dev/null   # open every anchor these real memories already declare

say "Right now the code matches what the memory describes."
run "gmr check '$COORD' && echo '${GREEN}nothing has moved${RESET}'"

# ================= SCENE 2 — the world moves =================
say "Now replay the real commit that later changed this function."
run "git show ${AFTER_REV}:$FILE > $FILE"
run "git --no-pager diff --stat HEAD -- $FILE"

say "This is a real, already-shipped change. The memory hasn't read it yet."

# ================= SCENE 3 — the catch =================
say "This is the part no linter and no eval tool can do."
cd "$WT"; in_worktree
type_line "gmr check '$COORD'"
sleep 0.3
if gmr check "$COORD"; then
  printf "%b\n" "${RED}(expected a non-zero exit here — check the demo setup)${RESET}"
else
  printf "%b\n" "${RED}✗ an axis this memory depends on moved — exit 1${RESET}"
fi
sleep "$BEAT"

# ================= SCENE 4 — resolve it on the record =================
say "You look, you decide, and the reason is sealed into the journal — quoting the real commit's own reasoning."
WHY="$(git -C "$WT" log -1 --format=%s -- "$FILE")"
run "gmr accept '$COORD' --baseline --why '$WHY'"

say "Settled again — because a person looked, not because nothing happened."
run "gmr check '$COORD' && echo '${GREEN}nothing has moved${RESET}'"

# ================= SCENE 5 — the point =================
say "Without this, your AI agent reads that stale memory and follows it. Confidently."
sleep 1.2
say "GMR — grounded memory runtime."
sleep 2.5

printf "\n%b\n" "${DIM}(recording can stop here — the worktree cleans itself up on exit)${RESET}"

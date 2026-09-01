#!/usr/bin/env bash
# GMR beyond code: a menu, an allergen, and both loops — memory and inference.
# Self-contained and offline: runs in a throwaway directory, never touches the
# repository it lives in. `--ci` drops the pauses and keeps the assertions.
set -euo pipefail

CI=0
[ "${1:-}" = "--ci" ] && CI=1

command -v gmr >/dev/null || { echo "gmr not on PATH"; exit 1; }
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

pause() { [ "$CI" = 1 ] || sleep "${1:-1.5}"; }
say()   { echo; echo "── $*"; pause; }
must()  {
  local want="$1"; shift
  set +e; "$@"; local got=$?; set -e
  if [ "$got" != "$want" ]; then
    echo "EXPECTED exit $want from: $* — got $got" >&2
    exit 1
  fi
}

cd "$WORK"
git init -q .
git config user.email demo@example.com
git config user.name demo
cp "$HERE/menu.json" .
git add . && git commit -qm "the menu as the kitchen serves it"

say "a repository with no code in it — just a menu. gmr init:"
gmr init >/dev/null

say "① the memory loop. The order page must warn nut-allergic customers,"
say "   and that duty rests on one JSON array:"
gmr anchor 'file://menu.json#$.items.2.ingredients' --as kung-pao-ingredients \
  -m 'The order page must show the nut-allergy warning for Kung Pao Chicken: it is cooked in peanut oil and finished with peanuts. If the ingredients change, this warning must be re-decided by a person, not silently kept or dropped.'
git add . && git commit -qm "anchor the allergen duty"

say "nothing has changed yet — check is quiet:"
must 0 gmr check

say "the supplier swaps the oil. Nobody thinks about the order page:"
sed -i.bak 's/peanut oil/sunflower oil/' menu.json && rm menu.json.bak
git add . && git commit -qm "supplier swap"

say "check — the value moved, and the memory comes back to a person:"
must 1 gmr check

say "a person re-reads it: peanuts are still in the dish, the warning stays."
say "accept seals that judgment:"
must 0 gmr accept kung-pao-ingredients --why 'oil swapped to sunflower, but whole peanuts remain in the ingredients — the allergy warning stays up'
must 0 gmr check

say "② the inference loop. An agent answers a customer, citing what it read:"
ADDR=$(gmr read kung-pao-ingredients --json | python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["fact_address"])')
must 0 gmr said 'told the customer the dish still contains whole peanuts, so the nut-allergy warning applies' \
  --on kung-pao-ingredients --saw "$ADDR" \
  --depends 'all(anchors, not state.v.value)'

say "standing — the conclusion is supported while its ground holds:"
must 0 gmr standing

say "the kitchen changes the recipe again — peanuts out entirely:"
sed -i.bak 's/, "peanuts"//' menu.json && rm menu.json.bak
git add . && git commit -qm "peanuts removed"
must 1 gmr check >/dev/null || true
set +e; gmr check >/dev/null 2>&1; set -e

say "standing — the ground moved, and the answer is no longer supported:"
must 1 gmr standing

say "That is the whole product, off the code path: a fact (a JSON array), a"
say "memory (the warning's rationale, handed back when the fact moves), and"
say "an inference (one answer, dead the moment its ground changed)."
say "Swap file:// for https:// or sql:// and nothing else changes."
echo
echo "walkthrough: every assertion held."

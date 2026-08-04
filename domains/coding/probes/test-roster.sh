#!/bin/sh
set -e
M=Cargo.toml
a=$(mktemp); i=$(mktemp); trap 'rm -f "$a" "$i"' EXIT
cargo test --workspace --manifest-path $M --no-run --quiet >&2
cargo test --workspace --manifest-path $M -- --list > "$a"
cargo test --workspace --manifest-path $M -- --list --ignored > "$i"
grep -q ': test$' "$a" || { echo "--list 一条测试都没列出来 —— 更可能是我读错了" >&2; exit 1; }
jq -n --rawfile all "$a" --rawfile ign "$i" '
  def roster: split("\n") | map(select(endswith(": test")) | rtrimstr(": test")) | sort;
  ($all|roster) as $t | ($ign|roster) as $g
  | { test_count: ($t|length), ignored_count: ($g|length), tests: $t, ignored: $g }'

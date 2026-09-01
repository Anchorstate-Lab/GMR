---
about:
  - console/cli/src/shapes.rs#vocabulary
  - console/cli/src/memories.rs#tombstones
watch: [sig, logic]
---

# The commit that deletes a word erects its headstone in the same breath

An anchor only puts a memory in front of a person when **the coordinate moved**. And
a memory naming a status, shape or field that no longer exists moves no coordinate at
all — `gmr check` is spotless, and that sentence just goes on being wrong.

Measured once: `e11bc73` deleted `Body::Table` and rewrote `delivery-standing.md`'s
three delivery paths in the same commit, and did not notice that the sentence lower in
the same section — "`captured` is settled; `added` `count-moved` `section-gone` are
not" — was by then pointing at things that did not exist. Four in one note. Seven
across the repository, spread over four notes.

## Why a headstone list, and not an assertion that things must exist

The other way round — scan the backticks in the memories and report any that are not
in the current vocabulary — cannot be made false-positive-free. There are over a
hundred backticked tokens in this repository's memories: `file` `name` `logic` are
live axes, `accept` `rebase` are verbs, `pub` `const` `use` are Rust. **In prose you
cannot tell which one is a vocabulary reference.**

A headstone list inverts that: it lists only **words that really were deleted**, with
no false positives. The price is one line on the day a word is deleted — and that day
is exactly when the window is open, with a person watching the word disappear.

## Why the exit code is 0

`moved-file` is **correct** in [[shapes-Dim]]: that whole section exists to record why
that axis was deleted. A memory is supposed to name what it buried. So "mentions a
retired word" is undecidable — report it, and let a person sort the headstones from
the ones somebody forgot to update. Same tier as `long-hand`.

`nothing_is_both_retired_and_shipping` keeps the list from fighting the vocabulary: a
word on both sides would get a correct memory reported as stale.

`vocabulary()` is `#[cfg(test)]` — "which words does this build have" is a question
only that one assertion asks, and nothing on the production path needs it. Making it
`pub` would add a public surface with no caller, and gate's `-D warnings` says so on
the spot.

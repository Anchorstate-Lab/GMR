---
about:
  - console/cli/src/shapes.rs#seen
  - console/cli/src/shapes.rs#state_carries_exactly_what_some_guard_compares_and_nothing_else
watch: [sig, logic]
---

# Only fields that take part in a criterion may live in the state

Once a capture has succeeded, the top level is these five: `position` · `baseline` ·
`now` · `v` · `status`. The field sets of `baseline` and `now` are identical to the
`field` of every `Since` axis, and the key set of `v` is identical to every axis
name. Not one more.

**`absent` is the only exception, and that exception is itself the criterion.** A
state that captured nothing has only `position` · `v` · `status` — this is not an
omission, `baseline` is **not allowed**: `baseline` means "the reading you
confirmed", and when the coordinate did not hit there is no such reading, so writing
one out of thin air pins a lie (see [[shapes-expand]]). So "not one more" and "absent
has two fewer" are two faces of the same discipline: the state can only house things
that genuinely took part in a criterion.

The live proof is in this repository: the top level of the state from
`gmr read 'doctrine::red-cards' --json` is exactly those three keys. Note that the
test pinning this walks only the captured path (`settled_state()` feeds
`obs.exact = true`), so no assertion watches the absent branch and this paragraph is
the only record on that side.

The reason lives over in the substrate: `should_still` compares **the whole State for
equality** (`crates/gmr-core/src/journal.rs`). So a field taking part in no guard
comparison still makes two readings' states differ — and a section moving down a line
writes a `Transitioned` with **no bit lit at all**. `gmr edges` fills up with
transitions that are not transitions while `gmr check` stays clean, because delivery
looks at the bit vector.

This is not imagined. `facts.line` came within an inch of getting in: the wish was
for `gmr status` to be able to show which line a section is on, so it went into
`reading()` without being given an axis. `body_lines` was the same, and was cut too.

**Position and size, the facts that are only for a human to look at, were not lost**:
they ride into the log with the observation, and `facts` is there every time. If it
is for a person to look at, take it from the observation; do not house it in the
state. Fill a rendering gap with rendering, not with state.

A corollary: wanting to add an axis means working out at the same time what it
compares against. `Now` axes (like `missing`) write no `baseline`/`now`, so they do
not join those two sets — what they say is "this reading is not about my target"; the
criterion is in [[shapes-Dim]].

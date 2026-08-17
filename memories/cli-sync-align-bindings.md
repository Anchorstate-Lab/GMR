---
about: domains/coding/cli/src/verbs/sync.rs#align_bindings
watch: [logic]
---

# `align_bindings` writes only when the binding relation actually changed

Binding is append-only in the store (see [[store-binding-record]]), so a
`bind` call that repeats an already-true relation still adds a new row
saying the same thing again. `align_bindings` computes `settled` — same
anchor set, same bound version — before deciding whether to call `rt.bind`
at all, specifically so that running `sync` repeatedly over an unchanged
repository does not grow the bindings table forever with identical
entries.

That is one of two things this function decides. The other — whether a note
that dropped one key and gained another is a rename or a typo — lives in
`ambiguous`, and is written up in [[cli-sync-rename-ambiguity]]. Both run
before `rt.bind`, and they refuse for different reasons: `settled` declines
to write a row that would say nothing new, `ambiguous` declines to write a
row nobody has authorised.

## When this changes, ask

Does the new code call `rt.bind` unconditionally instead of comparing
against the current binding first? Every sync run would then add a row,
even when nothing about the note's anchors or version actually moved.

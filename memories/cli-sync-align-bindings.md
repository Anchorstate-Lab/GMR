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

## The reference is the source's, not one this function builds

The address a note is bound at used to be assembled here, out of a global
constant naming one provider and the note's path. It is now
`note.reference` — the `Ref` the source stamped on the record it handed
over, carried through untouched.

The difference is invisible while one store exists, because a constant and
a carried value produce the same bytes. It stops being invisible the moment
a second source appears: the constant keeps naming the first one, so every
record from the second is looked up in a store that has never heard of it.
The symptom is this function refusing to bind anything, blaming a provider
that is working fine.

That is the shape of the defect, not just one instance of it — the same
"throw the answer away and re-derive it from a constant" appeared again in
the subscription lookup ([[delivery-standing]]) and in how a note's name was
spelled. A `Ref` handed down is the fix in all three.

## It resolves; it does not write

`align_bindings` returns a plan and performs no writes. The journal is
append-only, so "atomic" cannot mean rollback — it can only mean *resolve
everything first, and write the first row only once nothing can still fail*.

That distinction was not academic. With writes interleaved, a sync that
failed while versioning the last note left every anchor before it open and
nothing bound: 346 anchors, zero memories. Both `check` and `doctor` read
that state as fine — an anchor with no memory yet is what `gmr anchor`
without `-m` produces on purpose, so neither verb has grounds to complain.
The half-finished state is invisible precisely because each half of it is
individually legitimate.

There is a test asserting the plan is not written, and a second assertion in
it applying the plan and finding the binding afterwards. The second looks
redundant and is not: without it the first would pass just as well against a
fixture that could never bind at all.

It takes the declaring source for one reason: to name a note the way its
author does rather than by the address a store happens to keep it at. Every
other verb had already been moved to that ([[cli-notes-source]]); leaving
this one printing paths would have meant two verbs spelling the same note
differently, which teaches a reader to trust neither.

## When this changes, ask

Does the new code call `rt.bind` unconditionally instead of comparing
against the current binding first? Every sync run would then add a row,
even when nothing about the note's anchors or version actually moved.

Does anything here start constructing a `Ref` rather than cloning the one
the note arrived with? Whatever it constructs it from is a second opinion
about where a record lives, and the source already gave the first.

Does a write move back into the resolving loop — a `bind`, an `open`, a
settings change? Every one of them turns a later failure into a repository
half-synced, and this repository cannot see that state.

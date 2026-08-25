---
about:
  - crates/gmr-runtime/src/read.rs#AnchorView
  - crates/gmr-runtime/src/read.rs#MemoryView
  - crates/gmr-runtime/src/read.rs#read
watch: [sig, logic]
---

# A read exposes the same swap/staleness signals other verbs compute internally

`AnchorView.derivation` is what a caller compares against `instrument`'s
live resolution (see [[runtime-instrument]]) to notice a swapped probe from
the outside, the same signal `observe` uses internally to catch it.

`MemoryView.bound_at_seq` is the same field `BindingRecord` carries (see
[[store-binding-record]]) — the journal's position at bind time — taken
from the assertion that established the standing baseline rather than from
the newest one, for the reason in [[runtime-standing-baseline]].
`MemoryView.stale` is derived from it inside `read`, against *this*
anchor's `moved_at` rather than its head (see [[runtime-moved-at]]): a
bound-at seq behind the last entry that changed the state means the anchor
has moved since the binding was made, while an entry that merely failed or
restated the same value has not moved anything. `stale` stays
`None` when there is nothing to compare against, which includes every
record carried in via `MemoryLens::carry_linked` (see
[[runtime-carry-linked]]) — a linked-in record was never bound to this
anchor at all, so "moved since bound" has no meaning for it.

Everything about *whether the record still says what it said* lives in one
field, `MemoryView.grounding`, and is written up in [[runtime-grounding]].
`stale` and `grounding` answer different questions and neither implies the
other: `stale` is about this anchor moving, `grounding` is about the record
moving.

## A `MemoryView` carries how its assertions arose

`sources` is the set of `Source`s behind the reference's live assertions and
`asserted_at` the earliest of their times, so a reader can see whether
anything beyond the agent's own say-so stands behind the link — the question
[[store-binding-record]]'s `independent()` answers.

Both are per reference, because a view is. [[store-orset-projection]] can
leave several live assertions on one record, and a view per assertion would
show the same memory three times.

`asserted_at` is `Option` and skipped from JSON when absent: an assertion
with no recorded time has none to give.

## When this changes, ask

Is `view.stale` still computed only for records actually bound to this
anchor, never for ones carried in by a link? Computing it for a linked
record would compare against a head the binding was never actually made
against.

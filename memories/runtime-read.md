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
`MemoryView.warrant` is what `read` derives from it: whether the ground this
record was bound to still holds, decided by comparing the state as of that seq
against the state now, with `moved_at` only gating the comparison rather than
answering it — [[runtime-warrant]] is why that distinction is the whole design,
[[runtime-moved-at]] is why the gate is `moved_at` and not the head.

`warrant` is `None` on exactly the records that were never bound to this anchor:
`ground` fills it in while walking `bindings_on`, and `MemoryLens::carry_linked`
(see [[runtime-carry-linked]]) appends its records afterwards. A linked-in record
has no binding seq here, so "moved since bound" has nothing to mean.

Everything about *whether the record still says what it said* lives in one
field, `MemoryView.grounding`, and is written up in [[runtime-grounding]].
`warrant` and `grounding` answer different questions and neither implies the
other: `warrant` is about the fact underneath the record, `grounding` is about
the record itself.

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

Is `view.warrant` still computed only for records actually bound to this
anchor, never for ones carried in by a link? Today that is positional — filled
in during the `bindings_on` walk, before `carry_linked` appends — and an
ordering change is all it would take to start answering "the ground moved" about
a record that never stood on it.

This section described `MemoryView.stale` for the whole of this branch after the
field became `warrant`, and nothing caught it: the anchor was re-pinned and the
prose was not brought along. Re-pinning is what says "I looked"; it is not what
says "the words are still true".

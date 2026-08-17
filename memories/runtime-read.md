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
[[store-binding-record]]) — `None` unless the binding names exactly one
anchor. `MemoryView.stale` is derived from it inside `read`, relative to
*this* anchor's current head (`seq < s.head`): a bound-at seq behind the
head means the anchor has moved since the binding was made. `stale` stays
`None` when there is nothing to compare against, which includes every
record carried in via `MemoryLens::carry_linked` (see
[[runtime-carry-linked]]) — a linked-in record was never bound to this
anchor at all, so "moved since bound" has no meaning for it.

Everything about *whether the record still says what it said* lives in one
field, `MemoryView.grounding`, and is written up in [[runtime-grounding]].
`stale` and `grounding` answer different questions and neither implies the
other: `stale` is about this anchor moving, `grounding` is about the record
moving.

## When this changes, ask

Is `view.stale` still computed only for records actually bound to this
anchor, never for ones carried in by a link? Computing it for a linked
record would compare against a head the binding was never actually made
against.

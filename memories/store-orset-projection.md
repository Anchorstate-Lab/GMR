---
about:
  - crates/gmr-store/src/bindings.rs#Tag
  - crates/gmr-store/src/bindings.rs#Revocation
  - crates/gmr-store/src/sqlite/bindings.rs#gathered
  - crates/gmr-runtime/src/memory.rs#chain_from
  - crates/gmr-store/tests/conformance.rs#a_revocation_kills_only_the_tags_it_named
  - crates/gmr-store/tests/conformance.rs#a_revocation_does_not_reach_a_generation_it_was_not_made_at
watch: [sig, logic]
---

# Delivery is an OR-Set folded over a chain, and a revocation is a claim about named tags

One tag is one `(binding row, anchor)` pair. `bind` adds tags; `revoke`
records which tags it observed and kills. What is delivered under a
generation is every tag on that generation or its ancestors that no
applicable revocation named.

## Why the revocation names tags instead of flagging a row

Naming them is the whole mechanism. A later assertion of the same
coordinate lands on a **new** row, so it carries a tag the earlier
revocation never saw and survives. Flag the row instead — a `revoked`
column, a tombstone anchor — and a revocation becomes a permanent ban on a
coordinate rather than a claim about particular assertions. An agent that
re-derives a link the criteria now support could never say so again, and a
person's correction and an agent's next run would be fighting over one
mutable cell instead of both being on the record.

This is also why an assertion naming **no** anchor adds no tag and takes
none away. Writing one was how a record used to be detached, back when the
latest row replaced the whole set; removing something now means saying
which tags you observed, which is the only form a reader can audit.

## The chain comes from the runtime, and the store never asks for it

`bindings_on` takes `&[AnchorKey]` — the generation being read plus its
ancestors. `chain_from` walks `Anchor.supersedes` backwards, cycle-guarded
and capped at `GENERATIONS`, and hands the result over.

The walk cannot live in the store: `supersedes` is inside the journal's
`Entry::Open`, and `BindingStore` does not know journals exist. Reaching for
the journal from `SqliteBindings` would join two layers on the accident of
sharing a pool, and the in-memory implementation has no journal to reach for
at all.

**A revocation carries the generation it was made at, and the fold keeps
only revocations whose generation is in the chain being read.** So a
revocation made at an heir applies when reading the heir, and does not apply
when reading the ancestor alone — the assertion was correct for the criteria
that stood there, and a revocation made under later criteria says nothing
about it. That property holds mechanically, from the `IN` filter, rather
than by anyone remembering to check.

## Two directions, two questions, one answer each

`bindings_on(chain)` is delivery: read an anchor, hand the memories over.
`binding_of(reference)` is reconciliation: hold a memory, ask where it
stands. The second takes no chain because it is asked with no generation in
hand, so every revocation counts. These are different questions, not one
question with two answers.

## When this changes, ask

Does anything start deriving liveness from `binding.anchors` directly
instead of asking the projection? That reads the anchors as asserted and
misses both the revocations and the ancestors — the shape `corpus_health`
had, where a memory carried forward from a superseded generation counted as
nobody's, and the heir reported barren.

Does a revocation gain a form that does not name tags? Then check what
happens when the same coordinate is asserted again afterwards. If it stays
dead, add-wins is gone and with it the reason [[store-binding-record]]
records who asserted what.

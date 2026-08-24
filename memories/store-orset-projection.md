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

# Delivery is an OR-Set folded over a chain, and a revocation names tags

One tag is one `(binding row, anchor)`. `bind` adds tags; `revoke` records
the tags it observed and kills. What a generation delivers is every tag on
it or its ancestors that no applicable revocation named.

## A revocation names tags rather than flagging a row

Naming them is the mechanism: a later assertion of the same coordinate lands
on a new row, carrying a tag no earlier revocation saw, and survives.

A flag on the row instead makes a revocation a permanent ban on a coordinate
rather than a claim about particular assertions — an agent that re-derives a
link the criteria now support could never say so, and a person's correction
and an agent's next run would fight over one mutable cell instead of both
standing on the record.

An assertion naming **no** anchor therefore adds no tag and takes none away.
Removing something means naming the tags you observed, which is the only
form a reader can audit.

## The chain comes from the runtime

`bindings_on` takes `&[AnchorKey]` — the generation being read plus its
ancestors. `chain_from` walks `Anchor.supersedes` backwards, cycle-guarded
and capped at `GENERATIONS`.

The walk cannot live in the store: `supersedes` sits inside the journal's
`Entry::Open`, and `BindingStore` does not know journals exist. Reaching for
it from `SqliteBindings` would join two layers on the accident of a shared
pool, and the in-memory implementation has no journal at all.

**A revocation carries the generation it was made at, and the fold keeps
only revocations whose generation is in the chain being read.** One made at
an heir applies there and not to the ancestor read alone: the assertion was
correct for the criteria standing there, and a revocation under later
criteria says nothing about it. The `IN` filter is what makes that hold
without anyone checking.

## Two directions, two questions

`bindings_on(chain)` is delivery: read an anchor, hand the memories over.
`binding_of(reference)` is reconciliation: hold a memory, ask where it
stands. The second takes no chain — it is asked with no generation in hand —
so every revocation counts. Different questions, not one question with two
answers.

## When this changes, ask

Does anything derive liveness from `binding.anchors` directly instead of
asking the projection? That reads the anchors as asserted and misses both
revocations and ancestors, so a memory carried forward from a superseded
generation counts as nobody's and its heir reports barren. Reaching them at
all now means going through `Bound::assertions` ([[runtime-bound]]), which is
there for revocation and for nothing else.

Does a revocation gain a form that does not name tags? Check what happens
when the same coordinate is asserted again. If it stays dead, add-wins is
gone, and with it the reason [[store-binding-record]] records who asserted
what.

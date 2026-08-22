---
about:
  - crates/gmr-store/src/bindings.rs#BindingRecord
  - crates/gmr-store/src/bindings.rs#Asserted
  - crates/gmr-core/src/memory.rs#Source
  - crates/gmr-core/src/memory.rs#only_a_record_that_declared_itself_or_was_judged_stands_on_its_own
watch: [sig]
---

# An assertion carries how it came to be, and only two of the ways stand on their own

`Asserted` is what a caller hands the store; `BindingRecord` is what comes
back. Both wrap a `Binding` with the write-time metadata that is not part of
the relation itself — see [[memory-Binding]] for why that split exists at
all.

## `Source` is a fact about how GMR learned the link, not about the domain

The five words are `Derived` (the record declared its own coordinate, in
content that goes through review), `SelfAttested` (the agent that wrote or
used the record asserted it), `Adjudicated` (someone reviewed and affirmed
or revoked), `Configured` (a provider recipe declared it), and `Unknown`.

They live in `gmr-core` rather than the domain because the base is the one
holding the assertion, and the base is what answers `independent()`. Put the
vocabulary in the domain and every domain re-derives what counts as evidence
— which is the one judgement a reader is relying on this layer not to
invent.

**`independent()` is `Derived | Adjudicated`.** A memory whose aboutness has
only self-attestation behind it is the agent vouching for itself: worth
recording, because that is the most accurate moment the link can be made,
but not something a reader can weigh against the agent. `Configured` is
self-report with a longer life. `Unknown` is not counted, and that direction
matters — an assertion from before this column may well have been judged by
a person, and calling it independent would invent exactly the fact being
relied on. Under-crediting is the safe error here; over-crediting is not.

## `bound_at_seq` is only meaningful when there is one anchor to have a head

`Option<Seq>` rather than `Seq` because "the bound anchor's head at bind
time" only has one unambiguous answer when the binding names exactly one
anchor; a binding naming several has several heads, so there is no single
`Seq` to record and the field is `None`.

## The clock is the caller's

`Asserted` takes `at` as a field rather than reading `Utc::now()` where the
row is written. The store is handed the time the same way `Entry::Close`
is, so a replay puts back the moment the assertion was made rather than the
moment it was read.

## When this changes, ask

Does a new caller assume `bound_at_seq` is always `Some` for some anchor
count other than exactly one? A multi-anchor binding still has to leave this
`None` — inventing a `Seq` for it (first anchor's head? most recent?) would
silently pick one anchor's history as more important than the others.

Does a sixth `Source` arrive? Ask what it answers `independent()` with
before naming it. A word that has to be argued about is a word that will be
argued about differently by the next reader, and the whole point of splitting
by *kind of act* rather than by *who acted* is that no identity has to be
verified to tell them apart.

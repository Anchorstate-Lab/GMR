---
about:
  - crates/gmr-runtime/src/memory.rs#Bound
  - crates/gmr-runtime/src/memory.rs#by_claim
  - crates/gmr-runtime/src/memory.rs#bindings_on
  - crates/gmr-runtime/tests/grounding.rs#an_anchor_names_each_memory_once_however_many_assertions_stand_on_it
  - crates/gmr-runtime/src/bind.rs#bind
  - crates/gmr-runtime/tests/grounding.rs#an_assertion_that_says_what_already_stands_writes_nothing
  - crates/gmr-runtime/tests/grounding.rs#a_second_kind_of_assertion_on_the_same_link_is_not_a_repeat
watch: [sig, logic]
---

# `Bound` is the folded answer, and every reader is handed it instead of the rows

**Both directions of the projection return `Bound`**, not
`Vec<BindingRecord>`: `binding_of` one of them, `bindings_on` one per claim on
the anchor. The rows behind it stay reachable through
`assertions()` for the one caller that needs row identity — a revocation has
to name the tags it observed ([[store-orset-projection]]) — and nothing else
sees them.

A roster therefore cannot name a memory twice by forgetting to collapse
one. Three parties asserting a link is three assertions and one memory, and
a verb that lists per row reports three memories in trouble where there is
one — with nothing in its type to say it disagrees with the verb beside it.

`by_claim` groups through a map keyed by `Claim::identity`, not by scanning the
groups it has built so far. `corpus_health` hands it the whole table, so the
scan would be quadratic in the corpus rather than in one anchor's share of it.

The key is `identity`, not `Display`. Grouping on the rendered address had two
readings of one utterance land together only as long as nothing else rendered
the same way — and `said` was a legal provider name, so `said:t7` was both an
utterance and a record in a store called `said`. `check_provider_id` now refuses
that name, and this map keys on the same canonical form the store does
([[store-sqlite-ref-key]]). One thing decides claim identity, in one place.

Each dimension is folded exactly once, here:

- `anchors()` — the union across live assertions, which is the OR-Set
- `baseline()` — the newest assertion that cited a version, per
  [[runtime-standing-baseline]]
- `sources()` — the union
- `first_asserted()` — the earliest, because that is when the link was made

Handing out rows instead makes every caller a second implementation of this
fold, and only one of the four dimensions has semantics anybody wrote down.
The other three get decided per call site, differently, with the type
unchanged either way — so nothing reports the disagreement and delivery,
reconciliation and `doctor` end up answering the same question three ways.

## `says` is where write-idempotence lives, because the table is append-only

`says(anchors, version, saw, source)` asks whether the projection already holds
what a caller is about to assert. `Runtime::bind` asks it and returns
`Landed { recorded: false }` without writing when the answer is yes.

Nothing can be taken back from an append-only table, so a writer that
decides for itself whether its write is worth making has no way to be
checked and no way to stop. Every automated writer runs repeatedly by
design — `sync` on each pass, an agent on each thing it writes — and one
that re-states a standing relation adds a row that changes no field a reader
can see: same union, same baseline, same source set, same first-asserted
time. The only defensible test is whether the projection would move, and
only the projection can answer that.

`source` counts as part of what an assertion says. A second party asserting
a link a first party already asserted is new information — it is what
`independent()` reads ([[store-binding-record]]) — so it is recorded, and the
run after it is not.

So does `saw`. The same sentence asserted twice in front of two different
readings is two assertions, and collapsing them would throw away the only record
of which reading each answer was actually built from.

## `reaffirm` is deliberately outside this

`reaffirm` states no aboutness; it stamps a reading taken at a moment (see
[[runtime-reaffirm]]). Two readings of the same bytes at two moments are two
readings, so it writes through `MemoryLens::bind` directly. Routing it
through the guard would delete the only way to say "I have looked at this
again".

## When this changes, ask

Does a new caller take `assertions()` and fold a dimension itself? Whatever
it computes is a second answer to a question this type already answers, and
the type will not change when the two drift apart.

Does a new write path call `MemoryLens::bind` rather than `Runtime::bind`?
Then it is claiming to be a reading rather than an assertion, and it must be
one — otherwise it grows the table on every run of whatever drives it.

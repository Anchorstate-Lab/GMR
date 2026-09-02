---
about: crates/gmr-core/src/memory.rs#Binding
---

# The relation itself, versus one occasion of writing it down

`claim` says "about which anchors", full stop. Anything about **one particular
occasion of writing that relation down** — which content version was in view at
the time, which reading the asserter was looking at, when it happened — is
storage-layer view metadata, not part of the relation. Those live in
`gmr-store`'s `BindingRecord`.

The reason for the split: the relation is idempotent — binding the same claim to
the same anchors any number of times is one and the same fact — while "which time
it was bound, which version was seen" is ordered and accumulates. Mix them and
binding stops being idempotent.

`origin` is the one field that is neither aboutness nor occasion: it names the
utterance this relation condensed from, when it grew out of one. Ancestry of
the relation itself is part of the relation — every re-assertion of the
condensed claim carries the same origin, so it is idempotent the way `claim`
and `anchors` are, which is the test that put it here and not on `Asserted`.

That rule is what decided where `saw` went. It is the fact address the asserter
was in front of, and two assertions of the same sentence made a week apart were
in front of different ones — so it is a property of the occasion and it lives on
`Asserted`, not here. See [[store-binding-record]].

## A claim need not live anywhere

`Claim::Stored(Ref)` is a record in some store. `Claim::Said { id, asserts }` is
something an agent said, which is stored nowhere — the utterance is the claim,
and `id` is whatever the caller uses to point back at the turn it happened in.

The second exists because forcing an answer through a memory first was a layer
that bought nothing. From menu fact to answer, a written-down copy of the
sentence is one more thing that can drift from the sentence, watched by nothing.

`asserts` is what the utterance said, recorded and **not interpreted** — GMR
holds it so an auditor can read what was claimed beside what was true, and does
not decide whether one entails the other. It is decoration, not identity:
`Claim::identity` leaves it out, so one utterance is one claim however many
readings of it are filed. `Eq` does include it, because two `Binding`s that
differ there are genuinely different values; anything asking "same claim?" asks
`same`, and the store keys on `identity`.

## A stored claim spells itself exactly as the bare `Ref` did

`Serialize` for `Claim::Stored` emits the `Ref` object and nothing else, and
`Deserialize` accepts a bare `Ref` object; `Binding` carries
`#[serde(alias = "reference")]`. Both directions are load-bearing and neither
is politeness to old files: the bindings table keys a row by the canonical JSON
of what it is about and refuses `UPDATE` by trigger, so a spelling that moved
would strand every row already written with no way to bring it back. A test
asserts the two serialisations are equal, and another reads a pre-claim body.

## When this changes, ask

A timestamp, a version or a sequence number appears on `Binding` → it is turning
into a record. Ask: should this field stay unchanged when the binding is replayed?
If yes, it does not belong here.

Does a third `Claim` shape arrive? Ask what its `identity` is before anything
else — what makes two of them the same claim — because that is what the store
keys on and what `binding_of` looks up. And keep `Stored`'s spelling where it is.

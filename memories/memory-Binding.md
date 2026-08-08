---
about: crates/gmr-core/src/memory.rs#Binding
---

# The relation itself, versus one occasion of writing it down

`reference` says "about which anchors", full stop. Anything about **one particular
occasion of writing that relation down** — which content version was in view at
the time, when it happened — is storage-layer view metadata, not part of the
relation. Those live in `gmr-store`'s `BindingRecord`.

The reason for the split: the relation is idempotent — binding the same memory to
the same anchors any number of times is one and the same fact — while "which time
it was bound, which version was seen" is ordered and accumulates. Mix them and
binding stops being idempotent.

## When this changes, ask

A timestamp, a version or a sequence number appears on `Binding` → it is turning
into a record. Ask: should this field stay unchanged when the binding is replayed?
If yes, it does not belong here.

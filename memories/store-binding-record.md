---
about: crates/gmr-store/src/bindings.rs#BindingRecord
watch: [sig]
---

# `bound_at_seq` is only meaningful when there is exactly one anchor to have a head

`BindingRecord` wraps a `Binding` with the write-time metadata that is not
part of the relation itself — see [[memory-Binding]] for why that split
exists at all. `bound_at_seq` is `Option<Seq>` rather than `Seq` because
"the bound anchor's head at bind time" only has one unambiguous answer when
the binding names exactly one anchor; a binding that names several anchors
has several heads, so there is no single `Seq` to record and the field is
`None`.

## When this changes, ask

Does a new caller assume `bound_at_seq` is always `Some` for some anchor
count other than exactly one? A multi-anchor binding still has to leave
this `None` — inventing a `Seq` for it (first anchor's head? most recent?)
would silently pick one anchor's history as more important than the others.

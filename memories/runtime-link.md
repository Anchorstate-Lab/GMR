---
about: crates/gmr-runtime/src/link.rs#link
watch: [sig, logic]
---

# A link between two references says nothing about anchoring

`link` records a relation between two `Ref`s (`elaborates`, `contradicts`,
whatever `LinkKind` names) independently of `bind`. Linking `from` to `to`
does not imply either one is bound to any anchor, and binding a reference
to an anchor does not require it to be linked to anything. These are two
separate relations over the same `Ref` type, not one relation with two
views.

Both relations carry the same provenance axis: an edge is asserted with a
`Source` and revoked through `unlink`, which names only the live rows it
observed — so declaration reconciliation owns its `Derived` edges while an
agent's identical `SelfAttested` assertion stands, exactly the OR-set
discipline bindings live by ([[store-orset-projection]]).

## When this changes, ask

Does a new code path assume linking two references also affects, or
requires, their anchor bindings? The two relations have to stay
independently readable and writable.

---
about: crates/gmr-core/src/addr.rs#write_object
---

# Sorting the keys *is* the definition of the content address

`write_object` sorts the entries by key before writing them. That one line is the
foundation of the whole content-addressing scheme: two JSON documents that mean
the same thing but list their keys in a different order must hash to the same
value, or `ContentHash` is not the address of the content, it is the address of
one particular serialization.

With no features enabled serde_json uses a BTreeMap, which looks already sorted —
but that is **a coincidence that depends on a feature**, and `preserve_order`
turns it into insertion order.

**And this repository has it on.** The workspace root's `Cargo.toml` says
`serde_json = { version = "1", features = ["preserve_order"] }` outright, and
`cargo tree -p gmr-core --format '{p} {f}'` resolves to
`default,indexmap,preserve_order,std`. So `Map` right now **is** an IndexMap and
iteration right now **is** insertion order — this sort is not insurance against
someone turning something on later, it is the only thing currently holding up the
sentence "`ContentHash` is the address of the content".

## Having that feature on is why the tests can see this line

The direction of the risk is the opposite of the intuition. Both halves were
measured:

```
preserve_order on  + sort deleted -> 5 tests fail
preserve_order off + sort deleted -> everything passes, silently
```

Turn it off and `Map` goes back to being a BTreeMap, iteration is already sorted,
and deleting this line changes no output — `key_order_does_not_affect_output` ·
`nested_keys_sorted` · `content_hash_is_key_order_independent` ·
`whitespace_in_source_does_not_matter` ·
`canonical_form_is_pinned_against_library_drift` all keep passing. The code then
**silently** starts depending on BTreeMap's iteration order, which is the exact
coupling this line exists to avoid.

So insertion order is not the enemy this line guards against. It is its
**witness**.

## When this changes, ask

The sort is deleted, or replaced by whatever order the map itself iterates in →
do not ask whether `preserve_order` is on. It is, and the tests go red on the
spot.

**The thing actually worth watching is the reverse**: someone dropping
`preserve_order` from the workspace root's `serde_json` (it drags in indexmap, or
it disagrees with another crate). The moment they do, those five tests lose their
ability to discriminate — and they **will not go red**. A change that disables a
check looks exactly like a harmless dependency cleanup. Before dropping it, ask:
who is left watching content addressing?

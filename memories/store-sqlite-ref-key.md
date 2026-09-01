---
about:
  - crates/gmr-store/src/sqlite/mod.rs#ref_key
  - crates/gmr-store/src/sqlite/mod.rs#claim_key
  - crates/gmr-store/src/sqlite/mod.rs#keyed
watch: [sig, logic]
---

# `keyed` can `expect()` past the depth guard because what reaches it is flat

`canonicalize` can refuse input that nests deeper than its guard allows, but
`keyed` calls `.expect()` on that result without a fallback. Both callers hand
it something whose nesting depth is fixed by construction rather than by data:
`ref_key` a `Ref`, two flat string fields, and `claim_key` a `Claim::identity`,
which is that same `Ref` or the one-field object `{"said": id}`.

`Claim::Said` does carry an arbitrary `asserts` value, and it would nest as deep
as a caller likes. It never reaches here, because identity is not content —
`identity` leaves `asserts` out, which is also what makes two readings of one
utterance the same claim. See [[memory-Binding]].

`claim_key(Claim::Stored(r))` is `ref_key(r)`, byte for byte. Every binding
written before claims existed keeps its key, which is the only reason the change
was possible at all: the table refuses `UPDATE` by trigger, so a key that moved
would be a key with no way back.

## When this changes, ask

Does anything start keying on the whole `Claim` rather than on `identity`? Then
`asserts` reaches `canonicalize`, the depth guard becomes reachable, and this
`expect()` stops being provably safe — and separately, two readings of one
sentence begin filing under two keys that nothing looks up together.

Does `Ref` gain a field that could nest arbitrarily, rather than staying flat by
construction? Same answer: this needs a real error path.

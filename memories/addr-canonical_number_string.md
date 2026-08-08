---
about: crates/gmr-core/src/addr.rs#canonical_number_string
watch: [sig, logic]
---

# How a number is written is part of the hash

One value has many spellings: `1.50` `1.5` `1.5e0` `-0`. Canonicalization has to
collapse them into one, or two JSON documents with the same content hash to two
different addresses and `ContentHash` stops being the address of the content.

Four rules are nailed down here: integers go through `to_string()` and never
touch the float path; `-0` and `-0.0` are both written `0`; trailing `0`s after a
decimal point and a lone trailing `.` are dropped; `E` is always lowercased to
`e`.

Floats are formatted with ryu. **If ryu or serde_json quietly changes its format,
every historical hash in this repository stops matching** — the test
`canonical_form_is_pinned_against_library_drift` pins the bytes and the hash of
one fixed value so that kind of drift explodes when the dependency is upgraded,
not on the day someone compares against an old log.

That `unreachable!` at the end is not laziness: without `arbitrary_precision`,
`serde_json::Number` has only PosInt / NegInt / Float, and `as_f64()` is total on
all three. Turn that feature on and this line really is reachable — so it is an
invariant **that depends on a feature**.

## When this changes, ask

Any of the four format rules changed → every historical `ContentHash` is void and
logs cannot be compared backwards. That is not an "improvement", it is a breaking
change, and it has to be treated like swapping a probe version.

`arbitrary_precision` gets turned on indirectly by some dependency → the
`unreachable!` becomes a panic. Ask why it was turned on before deciding whether
to turn it off or to give this a real branch.

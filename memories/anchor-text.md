---
about: crates/gmr-core/src/anchor.rs#text
watch: [sig, logic]
---

# A rule's identity is its source text, hashed as a JSON string

`Expr::text` hashes rule source into a `ContentHash`, and what it hashes is
`Value::String(source)` — **not the raw bytes**. That way a rule's identity goes
down the same canonicalization path as every other content address, and the hash
of a rule table is comparable with the hash of a state.

That `.expect(...)` holds up: canonicalization only fails on structures that are
too deep or numbers that are not finite, and a string scalar is neither recursive
nor a number. This is an **impossibility guaranteed by the type**, not an
unhandled error.

## When this changes, ask

The hashed object changes from `Value::String` to something else (raw bytes,
salted, carrying a hash field) → every anchor's `declaration` hash changes and
`sync` reports every single anchor as criteria drift.

`content_hash_of` becomes able to fail on strings → that expect is now a panic
path and must become a `Result`. See [[addr-canonical_number_string]].

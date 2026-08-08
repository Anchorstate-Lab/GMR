---
about: crates/gmr-core/src/addr.rs#CanonicalizeError
---

# Canonicalization fails for its own reasons, not for IO's

Canonicalization can fail because of **something it did itself**: the structure
is too deep, a number does not round-trip. That is not what `io::Error` says —
that one says only that the end receiving the bytes refused them. Merge the two
into one error type and the caller can no longer tell "the value I handed you is
invalid" from "the disk is full".

## When this changes, ask

Does the new variant still describe only what **canonicalization itself** cannot
do? If a variant's cause comes from outside — network, file, lock — it does not
belong here. And when a variant is removed, ask the other direction: how is that
failure expressed now — has it truly become impossible, or has it been swallowed
into a panic?

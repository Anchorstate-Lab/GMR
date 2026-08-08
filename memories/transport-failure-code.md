---
about: crates/gmr-probe/src/lib.rs#from
watch: [sig, logic]
---

# `ProbeErrorCode` is deliberately smaller than `FailureCode`

`ProbeErrorCode` only enumerates outcomes a transport can actually produce
(unreachable, unusable, timed out, ...). `gmr_core::FailureCode` is bigger —
it also covers rule failures, which no transport raises; only the runtime's
rule evaluation can. This `From` impl is the one place the two vocabularies
meet, and it stays a narrowing conversion on purpose: widening
`ProbeErrorCode` to match `FailureCode` one-for-one would let a transport
claim a failure kind it has no way to have observed.

## When this changes, ask

Does the new `FailureCode` variant come from something a transport can
witness, or from rule evaluation? Only the former belongs as a new
`ProbeErrorCode` variant feeding this match.

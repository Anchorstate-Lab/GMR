---
about:
  - crates/gmr-runtime/src/translate.rs#code_of
  - crates/gmr-runtime/src/translate.rs#every_way_a_rule_can_fail_carries_its_own_code
watch: [sig, logic]
---

# The Fault-to-FailureCode translation lives here because gmr-expr cannot name the log

`gmr-expr` is a pure root (see the crate-boundary note in CLAUDE.md) and
has no compile-time dependency on `gmr_core`, so it cannot spell
`FailureCode` itself — `Fault` is as far as it can name its own failures.
`code_of` is the one place that maps each `Fault` variant onto the
matching `FailureCode`, living in `gmr-runtime` where both vocabularies
are already in view. The obs-strict/state-lenient split behind some of
these codes (`NoSuchField` on the obs side, `NewStateAbsent` on the state
side for the same shape of expression) is the same convention documented
in [[expr-changed]].

## When this changes, ask

Does a new `Fault` variant appear without a matching arm here? `code_of`
has to stay total over `Fault`, or a rule failure would have no
`FailureCode` to be recorded under.

---
about:
  - crates/gmr-expr/build.rs#main
  - crates/gmr-expr/build.rs#lockfile
  - crates/gmr-expr/build.rs#closure
watch: [sig, logic]
---

# The evaluator's version has to be earned, not asserted

`EVALUATOR_VERSION` is a hash, not a hand-written string, for the same reason
a probe's version is: a hand-written string can lie by omission — someone
changes comparison semantics or forgets to bump a constant, and the journal
goes on claiming it was the same evaluator when it was not. Hashing removes
the chance to forget.

Own source is not the whole closure. What two `Value`s compare equal to is
decided by `serde_json`, so the resolved version of the *runtime* dependency
closure is hashed in too (`closure`) — build-dependencies are excluded on
purpose, because nothing they do can reach a comparison at runtime.

`lockfile` refuses (panics) rather than falling back to "no lock found, skip
it": a version that quietly stopped covering its dependency closure would be
claiming a guarantee — "this hash tracks everything that can change the
comparison" — that it no longer keeps.

## When this changes, ask

Does the new code path change what two `serde_json::Value`s compare equal
as? If yes, its dependency (or its own source) belongs inside this hash, not
outside it — an evaluator that silently stopped covering an input it
influences is worse than one that never covered it.

---
about: crates/gmr-runtime/src/memory.rs#current_version
watch: [sig, logic]
---

# One path decides "current version," so stamping cannot disagree with reading

`current_version` is the single function that answers "what version does
this reference have right now" from a `ContentProvider`. Both version
stamping (`bind`/`reaffirm`, see [[runtime-bind]] and [[runtime-reaffirm]])
and reading (`read`, `edges`'s rewritten-standing check) go through it
rather than each calling `provider_for(...).fetch(...)` themselves. If two
call sites each looked up the provider and fetched independently, a change
to how a version is derived from a fetch could update one path and miss
the other, and a binding could be stamped as current by one path while
`edges` reports it as rewritten by the other.

`Runtime::current_version` is a wrapper, not a second path: it mints the
budget from the policy and delegates here. Callers reach for it so that
minting happens in one place rather than at every call site (see
[[content-budget]]).

## Two ways to have no version, and only one of them is an answer

`Ok(None)` means the provider was reached and has no such record — the
world's answer. A provider name nobody registered is `Err(NoProvider)`,
because that is an assembly fault in this binary, not a fact about the
record. Both used to come back as `Ok(None)`, and every caller then told
the user their record did not exist: a `gmr bind --provider mem0` in a
binary built without mem0 reported the uuid as missing rather than saying
it cannot reach that store at all. The same conflation, one layer up, is
what `Grounding`'s `Gone` and `NoProvider` keep apart for reading.

## When this changes, ask

Does a new caller that needs "the current version of this reference" call
`current_version`, or does it call `provider_for` and `fetch` directly?
Bypassing this function reopens the chance for stamping and reading to
disagree about the same reference.

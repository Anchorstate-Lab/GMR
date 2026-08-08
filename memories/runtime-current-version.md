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

## When this changes, ask

Does a new caller that needs "the current version of this reference" call
`current_version`, or does it call `provider_for` and `fetch` directly?
Bypassing this function reopens the chance for stamping and reading to
disagree about the same reference.

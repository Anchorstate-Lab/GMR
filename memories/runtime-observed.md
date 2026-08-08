---
about: crates/gmr-runtime/src/observe.rs#Observed
watch: [sig]
---

# `Observed` reports what a single observation did, at the resolution callers need

`Unchanged { state }` is distinct from `Still`: both mean the observed
world did not move the anchor's judged state, but `Unchanged` is what
happens when a full entry was still written (no rule matched, or the rule
matched but produced the same state) while `Still` means the write was
compacted into a back-reference instead — see `should_still` in
`gmr-core` for why that compaction exists. Collapsing the two into one
variant would hide from a caller whether a full entry actually landed in
the log.

`Attempt.attempts` is the streak length *after* this attempt, not before —
callers use it directly against `policy.stalled_attempts` without an
off-by-one adjustment.

## When this changes, ask

Does a caller need to tell "a full entry was written but nothing changed"
apart from "the write compacted into a `Still`"? If so, `Unchanged` and
`Still` have to stay separate variants, not merge into one "nothing
happened" case.

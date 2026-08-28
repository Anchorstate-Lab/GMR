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

`Contended` is neither a failure nor a look that changed nothing: the probe
answered, and the entry was not written because the log moved under the fold
more times than [[runtime-recorded]] will replay. Folding it into `Attempt`
would file a real reading as a failed one and start an exponential backoff
over a collision; folding it into `Still` would claim a look was recorded
when none was. What a caller does about it is its own third thing — come
back sooner, and do not count it against the anchor.

`Attempt.attempts` is the streak length *after* this attempt, not before —
callers use it directly against `policy.stalled_attempts` without an
off-by-one adjustment.

## When this changes, ask

Does a caller need to tell "a full entry was written but nothing changed"
apart from "the write compacted into a `Still`"? If so, `Unchanged` and
`Still` have to stay separate variants, not merge into one "nothing
happened" case.

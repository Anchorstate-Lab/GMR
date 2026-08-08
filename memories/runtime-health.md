---
about: crates/gmr-runtime/src/health.rs#health
watch: [logic]
---

# The rationale-size pass is separate from `scan` because `scan`'s callback is sync

`health` collects `rationale_hashes` inside the `scan` callback, then reads
each rationale's stored size in a second loop afterward, rather than doing
it in one pass. That split exists only because reading a sealed rationale's
bytes is `async` I/O and `scan`'s fold callback is not — the second loop
only ever iterates over the hashes the scan already picked out, it never
re-derives which revisions were restates.

## When this changes, ask

Does the new step still limit itself to iterating over what `scan` already
identified, or does it re-walk `entries` on its own? A second walk of the
log risks becoming the second projection [[journal-scan]] warns against.

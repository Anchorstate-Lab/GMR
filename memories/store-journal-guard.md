---
about:
  - crates/gmr-store/src/journal.rs#guard
  - crates/gmr-store/tests/conformance.rs#journal_refuses_a_stale_fencing_token
watch: [sig, logic]
---

# One shared token check, so the two backends cannot drift into disagreement

`guard` is a free function both storage backends call, rather than each
implementing its own fencing check — written separately, the two versions
would sooner or later refuse (or admit) writes differently, and a fencing
bug is exactly the kind of thing that stays invisible until two writers
actually collide.

Its second branch is not about staleness at all: once an anchor is under
lease management (`seen > 0`), no observation may be slipped in beside the
lease — that would be the second writer the lease exists to prevent in the
first place. `Fence::Unleased` is only refused here when the entry being
appended `is_sighting()`; author revisions go through unfenced even on a
leased anchor, because a human editing memory is not the concurrent-writer
problem this guard exists to catch.

## When this changes, ask

Does the new check still let author revisions through on a leased anchor
while still refusing unfenced observations? Collapsing that distinction
would either block legitimate edits or reopen the second-writer race.

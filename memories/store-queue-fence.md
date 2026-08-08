---
about:
  - crates/gmr-store/src/queue.rs#Queue
  - crates/gmr-store/src/queue.rs#lease
  - crates/gmr-store/src/sqlite/queue.rs#settle
  - crates/gmr-store/tests/conformance.rs#queue_contract
watch: [sig, logic]
---

# A `Queue` impl's fences must only ever climb, even across retirement

Every implementation of `Queue` has to guarantee that fences issued for one
anchor increase strictly monotonically, and that retiring an anchor does
not reset that counter. `journal::guard` (see [[store-journal-guard]]) uses
the fence as a high-water mark to block stale-lease writes — going
backwards even once, even after a retire, would let an old lease's writes
look current again and wedge that anchor's fencing forever, since there
would be no epoch left above the stale one to refuse it with.

`lease` exists specifically so a hand-triggered observation (someone asking
for a fresh reading right now, outside the normal due-queue cycle) still
goes through the same fencing path as a scheduled one. Without it, a
manual trigger could only write past the current token — which is exactly
the second-writer situation the lease mechanism exists to prevent. Not
getting the lease here means someone else already holds it, and the right
response is to let them write, not to retry around them.

`SqliteQueue::settle`'s `Disposition::Retire` arm is where the invariant
actually gets kept: it parks the row (`parked = 1`) instead of deleting it,
because `epoch` is this anchor's token high-water mark, and deleting the
row would let a future `INSERT` restart the count from zero — exactly the
backwards jump the trait's contract forbids.

## When this changes, ask

Could a new backend, or a retry/retire path, ever reissue a fence number
that is not strictly greater than every fence issued before it for that
anchor? Any way to make that happen breaks the high-water-mark guarantee
every `journal::guard` call relies on. In particular: does retiring an
anchor ever delete its row rather than parking it?

---
about:
  - crates/gmr-probe/src/lib.rs#resolve
  - crates/gmr-probe/src/lib.rs#invoke
watch: [sig, logic]
---

# `resolve` answers identity before `invoke` answers the world

`Transport::resolve` has to be answerable without taking a reading: it tells
you what a name stands for from declaration alone, so a bad probe name is
refused before anything runs, and so an instrument swap is knowable by
comparing derivations, never by comparing two live readings against each
other. `invoke` is the only method allowed to touch the world; the identity
it stamps on the answer always came from a prior `resolve`, never from the
call itself.

## When this changes, ask

Would the change make `resolve` need to run a probe to answer? That collapses
the split this trait exists to keep — identity must stay cheaper than a
reading.

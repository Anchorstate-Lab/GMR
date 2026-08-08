---
about: crates/gmr-core/src/journal.rs#scan
---

# There is only one projection

`scan` walks the log once and hands out the fold as it stood after each entry.
`fold` is simply its last cell.

Consumers that need to know "what happened along the way" come here. **Do not
write a second projection.** Two projections drift apart sooner or later, and
nothing will notice — because each is self-consistent on its own, and the only way
to see it is to feed the same log to both, which nobody does.

This one is first-principles: current state can only come from a projection of the
log, so there can only be one projection.

## When this changes, ask

A second function appears that walks `entries` and rebuilds state → whatever it is
called, it is a second projection. Ask it why it cannot be a callback of `scan`.

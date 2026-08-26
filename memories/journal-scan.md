---
about:
  - crates/gmr-core/src/journal.rs#scan
  - crates/gmr-core/src/journal.rs#resume
---

# There is only one projection

`scan` walks the log once and hands out the fold as it stood after each entry.
`fold` is simply its last cell.

There are three names here and still one projection. `resume` is the loop;
`scan` is `resume` starting from nothing, and `fold` is `scan` with the callback
thrown away. Adding a seed did not add a walk — it made explicit that the walk
always had an accumulator, and that starting it from `None` was a choice rather
than a law.

That seed is what lets a caller keep a checkpoint instead of re-reading a log
from the beginning to learn what it already knew (see [[runtime-assembly]]), and
it is sound for a reason visible right here: every arm of this loop reads only
the accumulator and the entry in hand. Nothing reaches back for an earlier
entry — `Entry::Still` carries `ref_entry` and pointedly does not follow it. A
test folds a log of every entry kind, cutting it at each seq, and asserts
resuming from that cut lands exactly where folding the whole thing lands.

**An arm that reached backwards would break that silently.** It would keep every
fold-from-zero test passing, because from zero the earlier entries are all
present. That is the one change here that has to be noticed, so it is the
question at the bottom of this note.

Consumers that need to know "what happened along the way" come here. **Do not
write a second projection.** Two projections drift apart sooner or later, and
nothing will notice — because each is self-consistent on its own, and the only way
to see it is to feed the same log to both, which nobody does.

This one is first-principles: current state can only come from a projection of the
log, so there can only be one projection.

## When this changes, ask

A second function appears that walks `entries` and rebuilds state → whatever it is
called, it is a second projection. Ask it why it cannot be a callback of `scan`.

An arm starts reading something other than the accumulator and the entry in hand
— following `ref_entry`, indexing into `entries`, looking at a neighbour → the
fold has stopped being resumable, and every caller holding a checkpoint is now
folding from a state that arm could not have produced. Nothing fails from zero,
so nothing fails. Either keep the arm local or retire the checkpoints with it.

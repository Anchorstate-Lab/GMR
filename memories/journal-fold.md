---
about: crates/gmr-core/src/journal.rs#fold
---

# `closed` accumulates, it is never re-read

`closed` accumulates one entry at a time and never clears. Closure is **something
that happened in the log**, not a fresh reading of the final state.

Here is the difference: recompare the final state against the terminal set every
time, and "entered a terminal state, then got moved out by a Restate" reads as
"never finished" — the history has been erased. Accumulating does not do that.

The `||` in `s.closed = s.closed || s.anchor.is_terminal(&s.state)` is this
sentence.

## When this changes, ask

`closed` becomes able to go from true back to false → that violates decision 8
directly. Any refactor that "recomputes closed" has to answer first: is the fact
that it once entered a terminal state still visible?

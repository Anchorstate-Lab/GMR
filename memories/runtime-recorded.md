---
about:
  - crates/gmr-runtime/src/observe.rs#recorded
  - crates/gmr-runtime/src/observe.rs#REPLAYS
  - crates/gmr-runtime/tests/operations.rs#no_entry_is_ever_folded_from_a_state_something_else_already_replaced
  - crates/gmr-runtime/tests/operations.rs#a_replay_does_not_put_out_a_bit_the_other_writer_just_lit
watch: [sig, logic]
---

# A state that moved under us is arithmetic to redo, not a reason to look again

`looked_at` ends at the `Observation`. Everything after it — evaluate the
rule table, decide `Still` against `Transition`, append — is `recorded`, and
it is a loop because the append can be refused: the head it folded against
may have moved ([[store-journal-expected]]).

**The probe is outside the loop and stays outside it.** `transition` is a
pure function and the `Observation` is already in hand, immutable, carrying
its own fact address and instrument version. Nothing about the world became
unknown; only the state the arithmetic was done against did. Going back to
the network here would spend a real call to learn something we already knew,
and would make contention cost what an observation costs.

## `should_still` has to be recomputed too, and that is what makes it free of duplicates

Retrying only the `append` would write a second entry for one transition —
that would be the obvious shape and it is wrong. `should_still` reads
`s.state` and the previous fact address, so it is part of the arithmetic. On
the replay it is asked again against what the other writer actually left: if
that entry carries the fact address we just measured, the recomputation
lands on `Still` with no attempts behind it, and **nothing is appended at
all**. The de-duplication is not a special case anybody wrote; it falls out
of redoing the whole computation rather than half of it.

## Three things heal themselves, one is recorded and caught downstream

The loop re-reads `s.anchor`, so a `Revise` landing mid-flight mostly
resolves itself: new rules, new state or a new terminal set are simply what
the recomputation uses. A `Close` is seen as `closed` and returns
`Observed::Closed` without writing.

`Change::Reprobe` is the one that does not heal — our `Observation` came
from the old instrument and its `versions.declaration` says so. It is
written as it is, honestly, because the reading was real; downstream
`Holding::Incomparable` exists precisely to notice that what was taken and
what is now read came from different instruments.

## Giving up writes nothing, and nothing durable is lost

Replays exhausted leaves `Observed::Contended`. Losing the write does not
lose the signal: an accumulating axis compares against `state.baseline`,
which only `accept` moves, so the next observation that lands sees the same
difference and lights the same bit. A `Now` axis is recomputed from the
reading every time and cannot be consumed at all. What a caller sees in the
meantime is the older `taken_at` that is genuinely in the log — see
[[runtime-grounding]] for why saying so is the only honest option.

## When this changes, ask

Does anything inside the loop reach a probe, a clock, or a store other than
the journal? The loop's whole claim is that it is pure arithmetic plus one
append, and a second source of truth inside it means two replays could
disagree.

Does the recomputation still cover **every** decision that reads `s`? A
value computed before the loop and used inside it is folded against a state
that may no longer hold — which is the bug this exists to refuse, arriving
from the inside.

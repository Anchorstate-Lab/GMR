---
about:
  - crates/gmr-core/src/journal.rs#Faltering
  - crates/gmr-core/src/journal.rs#AnchorState
watch: [sig, logic]
---

# A run of failures is one value, because the count and the reason die together

`AnchorState` used to carry `attempts: u32` and nothing else about the run. The
count answered *how many*, and no folded state answered *which failure* — the
code and the message on `Entry::Attempt` were read for their side effect on the
counter and then dropped.

`Faltering` carries both, as one value, and `attempts()` is derived from it. The
alternative — a `u32` beside an `Option<Faltering>` — makes `attempts == 0` with
a reason still attached representable, and `attempts == 2` with no reason too.
Neither means anything, and nothing would reject them.

That is exactly the shape [[runtime-grounding]] was written about: `MemoryView`
once carried five `Option`s that could contradict each other, and did. This is
the same mistake one layer down, so it gets the same answer — one value that is
always exactly one of the things that can be true.

**The two halves share a lifecycle, which is what earns them one field.**
`Open`, `Transition` and `Still` all clear the run; `Attempt` extends it. There
is no entry that resets the count while leaving the reason, or the reverse. Two
fields would be two spellings of one clock.

`attempts()` is a method rather than a stored field for the reason
[[journal-reason]] gives: whatever is derivable from facts already held does not
get stored a second time.

## Who could not answer before this

`check` reads the code off a live `Observed::Attempt`, so it was never blind. The
blind caller is anything answering from folded state without observing afresh:
it knew a streak existed and could not say whether the source could not be
reached, the artifact was unusable, or the rules could not be evaluated — and
[[layers]] makes those three different people's problem. `scan` could see the
reason as it passed each entry, but only transiently and only while walking;
`fold` returns the accumulator alone.

## Entries already on disk

The question [[journal-FailureCode]] insists every new field answer. `Faltering`
never touches the disk: the journal stores `Entry`, and `AnchorState` is the
fold's product, recomputed on every read. `Faltering::code` is `Option` only
because `Entry::Attempt.code` is, and for the same append-only reason.

## When this changes, ask

Does a field appear beside `faltering` that describes the same run? Then the run
has two spellings again, and the next question is which one a reader is supposed
to believe.

Does something want the reason to survive a sighting? That is not a run of
failures any more — it is a history, and the journal already is one.

---
about:
  - crates/gmr-store/src/journal.rs#Expected
  - crates/gmr-store/src/journal.rs#Journal
watch: [sig, logic]
---

# A write carries the premise it was computed from, not just permission to write

`Entry::Transition` is `f(state@H, obs)`. For a long time `append` never
learned what `H` was: the signature had nowhere to put it. `AnchorState`
had already worked the number out, `stood()` handed it to the runtime, and
it was dropped on the floor at the call. **A log cannot keep an invariant
nobody ever tells it**, so it kept the one it was told — who may write —
and not the one that decides whether the entry is right.

Those are two orthogonal questions and they need two answers:

```
am I still a legal writer?          lease + fencing token   pessimistic, sized for a crash
does what I computed from hold?     Expected::Head(seq)     optimistic, one comparison
```

`Expected` is deliberately a separate parameter and not a third `Fence`
variant. [[store-journal-fence]] records what goes wrong when two situations
that must be refused for different reasons share one value; folding a
premise into a permission would be that mistake made a second time.

## `Any` is not "do not care"

`Any` says **this entry's content was decided by nothing it read**. There is
exactly one such entry today: `Attempt`. Its `attempts` count is worked out
at fold time (`s.attempts() + 1`), never stored, so two concurrent failures
each counting once is the right answer and there is no premise to break.
Everything else states a head, including author revisions — a `Revise` seals
an immutable rationale derived from the state it read, and "the anchor moved
while you were writing the reason" is something to be told, not something to
overwrite in silence.

`Open` states `Head(0)`. That was the worse of the two bugs this closed:
`fold` replaces its accumulator outright on a second `Open`, so two
concurrent opens did not leave a duplicate entry — they silently discarded
every observation and every accumulated bit since the first one.

## Where it is checked, and why that costs nothing

Inside the `BEGIN IMMEDIATE` that `append` already opened, on a write lock
already held, as one more query of the same shape as the fence read it
replaced. There is no new contention surface and no new failure mode.

## When this changes, ask

Adding a variant → what does it let a caller decline to state, and can that
caller's entry be decided by something it read? If yes, it is `Head`.

Adding an entry kind → is its content derived from a read? Almost always
yes; `Any` is the exception, and it has to be argued for, not defaulted to.

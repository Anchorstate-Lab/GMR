---
about: crates/gmr-core/src/journal.rs#Versions
---

# The three identities behind one observation, never to be merged

`declaration` (what the anchor wrote down), `derivation` (what actually derived
these facts), `evaluator` (the evaluator at the time) — three things that evolve
independently and fail in different ways. Merge any two and you are lying about
the third.

Concretely: the probe script changed but the anchor did not → only derivation
moved. The anchor named a different probe → only declaration moved. The evaluator
was upgraded → only evaluator moved. Every reading of "the state moved" starts by
ruling out which of these three moved. Merged, you cannot rule anything out.

## When this changes, ask

Someone wants to squash the three into one "version" → ask them: the probe changed
and the anchor changed, how do you plan to tell those apart? Phase B's multi-probe
work pushes declaration and derivation down into each reading and leaves evaluator
at the observation level — that is splitting finer, not merging.

## What each of the three is

```
declaration   what the anchor wrote down
derivation    what actually derived these facts, and whether that identity is provable
evaluator     the evaluator that was running at the time
```

The second one carries a `Verifiability` of its own (see [[probe-Verifiability]])
— "what derived it" and "how much that claim can be trusted" are two things too,
and they were not merged either.

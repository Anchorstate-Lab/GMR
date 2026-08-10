---
about:
  - crates/gmr-probe/src/lib.rs#Budget
  - crates/gmr-probe/src/lib.rs#narrowed
  - crates/gmr-probe/src/lib.rs#ProbeCall
  - crates/gmr-runtime/src/policy.rs#budget
  - crates/gmr-probe/src/lib.rs#narrowing_can_only_tighten_a_budget_never_widen_it
  - batteries/survey/src/cache.rs#scan
  - batteries/survey/src/cache.rs#Halt
  - domains/coding/extract/src/lib.rs#every_probe_stops_when_nobody_is_waiting_for_it_any_more
watch: [sig, logic]
---

# A budget is an absolute deadline, because a relative one multiplies

`Budget` holds an `Instant`, not a `Duration`, and that is the whole design.
`pass` leases a batch and observes it one anchor after another. Had each call
carried "thirty seconds" the batch would have been worth thirty seconds times
however many tickets `due()` returned — sixty four of them by default, which is
half an hour wearing the label of thirty seconds. Minting one `Budget` before
the loop and handing the same one to every anchor makes a batch cost what it
says it costs. This is why gRPC and tower propagate deadlines rather than
timeouts; the reason is the same and so is the shape.

`narrowed` is how a per-anchor budget composes with the batch's: it takes the
**earlier** of the two deadlines, so an anchor asking for an hour inside a batch
worth fifty milliseconds gets fifty milliseconds. A per-anchor knob may tighten
and may never widen — otherwise it is not a knob, it is a way around the bound
the batch already committed to. It does hand out a fresh cancellation flag,
because one anchor giving up must not cancel the rest of the batch with it.
Both halves are asserted, because both are easy to write the other way round.

## What a budget may and may not decide

> A budget may only ever produce **no answer**. It must never produce a
> **shorter answer**.

This is the line that keeps it out of the earned version. A deadline that could
yield a partial roster would be changing what was observed, and then it would
have to be hashed into every extractor's closure — and retuning it from thirty
seconds to forty five would ask every repository in the world to rebase for a
reading that did not move. Because it can only refuse, it changes no fact, and
`Entry::Attempt` — which is what a spent budget produces — carries no `Versions`
at all. There is nothing to hash it into, and that is not an oversight.

The same rule is why `Reach` carries it: the work needs to see the deadline to
stop, and seeing it must not change what the work would otherwise have said. See
[[transport-inprocess]] for the other side of that line, and [[survey-narrow]]
for the same distinction drawn about an optimisation.

## A deadline nobody looks at is not cancellation

For a while `checkpoint()` had exactly one caller in the whole repository, and
it was in a test. The transport called `budget.cancel()` on timeout, the flag
flipped, and the blocking thread scanning the repository never read it — so it
ran to natural completion exactly as it had before any of this existed. What
`Budget` actually delivered was **deadline propagation**, which is real and is
most of the value; cancellation was decorative.

The test that made it look wired is the dangerous part. `work_that_outran_its_
budget_is_told_nobody_is_waiting` proves the mechanism works *when something
cooperates*, and nothing in production did. A green test over an unreachable
mechanism is worse than no test, because it answers the question nobody then
asks again.

The checkpoint now lives in `scan`'s walk, once per file — the one loop every
extractor goes through, so no probe has to remember. Reaching it cost the budget
a parameter on all four `probe()` functions and therefore **one version bump per
probe**: unavoidable, because the closure hashes those files' bytes, and the
alternative (an ambient deadline in a thread-local or a slot on the shared
`Cache`) is the implicit clock and shared mutable state the discipline forbids.
The answers were checked byte-for-byte across all four probes on a frozen
snapshot first, so the bump is a translation.

`every_probe_stops_when_nobody_is_waiting_for_it_any_more` is the guard, and it
loops over the fixture table rather than naming probes, so a fifth extractor is
covered the day it gets a fixture. It was checked red with the checkpoint
removed.

## A spent scan is not remembered, a refused one is

`Halt` splits the two, and `worth_remembering` acts on the split: a
`Refused` scan is deterministic — the same corpus refuses the same way — so the
flight memoises it and the rest of the pass is spared a repeat. A `Spent` scan
says nothing about the corpus at all; memoising it would poison the scope for
the life of the process because one anchor ran late. This distinction is only
expressible because the error stopped being a `String`.

## `ProbeCall` is a struct because the next thing will ride along

`Transport::invoke` takes one request rather than a widening list of positional
arguments. The trait is re-exported from the facade, so every change to it is a
public break; a struct spends one break on making the next addition — a trace
context, an idempotency key, a caller identity — cost a field instead of another
break.

## When this changes, ask

Does anything now let a budget shorten an answer rather than refuse it? That is
the moment it stops being operational and becomes a derivation input, and it
would have to enter the closure — with everything that costs. And can a
per-anchor or per-call budget still only tighten what it was handed? A single
`max` where a `min` belongs turns the batch bound back into a suggestion.

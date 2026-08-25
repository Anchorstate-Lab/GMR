---
about:
  - crates/gmr-probe/src/lib.rs#Budget
  - crates/gmr-probe/src/lib.rs#narrowed
  - crates/gmr-probe/src/lib.rs#ProbeCall
  - crates/gmr-runtime/src/policy.rs#budget
  - crates/gmr-probe/src/lib.rs#narrowing_can_only_tighten_a_budget_never_widen_it
  - batteries/survey/src/corpus.rs#rescan
  - batteries/survey/src/corpus.rs#Halt
  - batteries/survey/src/corpus.rs#deterministic
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

### The third field that is not here

`Budget` was designed with a `file_cap` beside the deadline and the
`output_cap`: a per-file byte ceiling, with files above it skipped instead of
read. It was not dropped for lack of time. **The rule above rules it out.**
Skipping a file removes every candidate that file would have contributed, and a
run with the cap then answers a question a run without it would have answered
differently — a shorter roster, not a refusal. That is a knob that changes the
answer sitting outside the earned version, which is the one thing this section
exists to forbid.

`output_cap` looks like the same idea and is its opposite: it measures the
finished answer and rejects **the whole of it** (`ProbeError::too_large`). It
can only ever produce no answer, so it stays operational and out of the closure.
The test is not "does it have a number in it" but "can the result still be
handed to a caller after it fires".

So nothing refuses a large file today — `visit` reads whatever it walks over,
and a single 5.6 MB source file costs about a second here, all of it inside one
uninterruptible gap between two checkpoints. If a ceiling is ever wanted, it
belongs in the extractor's `eligible()`, alongside the extension rules: a
statement about which files this probe looks at, hashed into its closure, so
narrowing it earns a new version and every anchor rebases. That is the honest
price of changing what was looked at, and the reason it cannot be bought with an
operational flag.

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

The checkpoint lives in `rescan`'s walk, once per file — the one loop every
extractor goes through, so no probe has to remember it. That is why the budget
is a parameter on every `probe()` function: an ambient deadline in a
thread-local, or a slot on a shared corpus, is the implicit clock and shared
mutable state the discipline forbids, and it would hide which reading the
deadline belonged to.

`every_probe_stops_when_nobody_is_waiting_for_it_any_more` is the guard, and it
loops over the fixture table rather than naming probes, so a fifth extractor is
covered the day it gets a fixture. It was checked red with the checkpoint
removed.

## Three ways a reading stops, and only one is a fact about the corpus

`Halt` splits them, which is the point of it being a type rather than a
`String`:

```
Spent      the deadline that ran out was one caller's        not-knowing
Faulted    the index would not answer                        our failure
Refused    this corpus makes no sense to this recipe         the answer
```

Only `Refused` is a property of `(tree, recipe)`, and `Halt::deterministic`
is that predicate. It is what decides whether an answer may be remembered on a
later caller's behalf — see [[bridge-Bridge]].

Collapsing `Faulted` into `Refused` is the specific mistake to avoid: a lock
held for a moment is not a corpus that makes no sense, and anything that caches
refusals would make that moment permanent.

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

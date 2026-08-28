---
about:
  - crates/gmr-runtime/src/pass.rs#pass
  - crates/gmr-runtime/src/pass.rs#Passed
  - crates/gmr-runtime/tests/operations.rs#a_batch_that_runs_out_of_budget_does_not_blame_the_anchors_it_never_reached
  - crates/gmr-runtime/tests/operations.rs#an_anchor_the_budget_never_reached_comes_back_at_the_front_of_the_next_pass
watch: [sig, logic]
---

# Not being looked at is not a failing grade

A batch mints one `Budget` and hands the same one to every anchor in it, so
that a batch of sixty four costs what it says it costs — see [[probe-budget]].
The consequence nobody wrote down is that the batch's cost is a **sum**, and a
queue whose anchors add up to more than the budget has a tail that the clock
never reaches.

Before this, `pass` walked the whole ticket list regardless. Every anchor past
the deadline was still invoked, the transport immediately answered with a spent
budget, and `observe` filed it through `record_attempt(s.attempts + 1)`. So an
anchor that was **never looked at** collected an attempt, an exponential
backoff, and — on the third pass — a `Stalled` edge from
[[runtime-edges-walk]]. The system announced "this anchor is stuck" where the truth
was "we ran out of time before its turn". That is the one thing this project
exists not to do.

`pass` now checks the budget before each ticket and, when it is spent, settles
the rest as `Reschedule { after_secs: 0 }` without observing them at all.

`Observed::Contended` settles the same way and for the same reason. The
probe ran and answered; the entry lost a race for the head
([[runtime-recorded]]). Backing that off would be the identical mistake in a
new place — announcing that an anchor is struggling when what happened is
that somebody else wrote first.

## Zero, not a cadence

The skipped anchor is still due — nothing about it was answered. Rescheduling
it a cadence away would let a batch that is permanently too small for its queue
look healthy forever: the tail would starve one cadence at a time and every
individual pass would look fine. At zero it sorts ahead of the anchors this
pass did observe (`due` is `ORDER BY due`, and an observed anchor gets pushed
out by its cadence), so the next pass starts where this one stopped.

## Why the count is on `Passed` and printed

A pass that got through a third of its batch and a pass that had nothing left
to do are the same two lines of output without it, and `observed == 0` was
printing "nothing was due" while a full queue sat untouched. `skipped` is the
number, and the human line says what to do about it, because the operator's
next move — raise `--probe-budget-ms`, or accept a smaller batch — is not
derivable from a number alone.

## The invariant the test actually pins

> A batch that runs out of budget can produce **at most one** timed-out anchor:
> the one in flight when the clock ran out.

Everything behind that one was never invoked. This is stronger and steadier
than counting who succeeded — it holds whatever the machine's timing does, and
it fails loudly if anyone reintroduces the invoke-anyway path. The companion
assertion is that `observed + skipped` accounts for every due ticket, so no
ticket can be quietly dropped instead.

## When this changes, ask

Does anything in the loop reach a probe without first asking whether the budget
is still alive? And does every path out of the loop settle its ticket — a
`continue` that skips `settle` leaks the lease and the anchor goes quiet for
`lease_secs` with no record of why.

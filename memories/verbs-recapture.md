---
about: console/cli/src/verbs/mod.rs#recapture
---

# Re-pinning a baseline means going and looking again, not pinning last time's reading

Clear the state back down to `position` alone and let the shape's own capture rule run
again — this action used to have three implementations: one in `rebase`, one in
`accept`'s table branch, one in `accept`'s vector branch. The first two were copies;
the third was invented, and only the third was wrong: it pinned `state.now`, and that
is **what the previous δ wrote in**.

The consequence was measured: break something → `check` (red) → revert the change →
`accept` → `check` ⇒ **still red**. A stale reading had become the new baseline, so good
code was the thing that "changed", and it took two accepts to get back.

The right way was already written down in [[delivery-standing]]; it had just only been
applied to table shapes:

> Clear the state back down to position alone and let the shape's own capture rule run
> again. If it cannot capture, it says `absent` honestly, and accept does not paper over
> the problem.

The same holds for vector shapes — R1 `not exists(state.baseline) and obs.exact` is the
capture rule, and when the target is gone R2 says `absent` instead of pinning a lie.

**The fix was not "add another observe", it was three implementations becoming one.**
The duplication itself is what caused this bug: two were copied right and one was not,
and nothing would ever have found it.

## When this changes, ask

`observe` gets taken out of here → we are immediately back to pinning last time's
reading. Ask: which observation wrote that reading?

A fourth place starts assembling its own `Restate { state: {position} }` → that is a
fourth implementation. Ask it why it cannot call this one.

The order cannot be reversed: `revise` first, then `observe`. The other way round
observes the old baseline once first, adding a meaningless transition to the log. These
remain **two independent log entries** — δ's inputs have not changed (decision 7);
observation is observation, and a change of criteria is a change of criteria.

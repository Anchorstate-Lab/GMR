---
about:
  - domains/coding/cli/src/verbs/said.rs#run
  - domains/coding/cli/src/verbs/standing.rs#run
  - domains/coding/cli/src/verbs/standing.rs#counted
  - domains/coding/cli/src/verbs/standing.rs#exit_of
  - crates/gmr-runtime/src/bind.rs#claims
watch: [sig, logic]
---

# This repository supervised its own memories and not its own conclusions

Every verb here served the memory loop: code moves, `check` hands a note back, a
person re-reads it and seals a reason. The other loop [[three-layers]] names —
an agent concludes something for the task in front of it — ran through this
repository dozens of times a day and left nothing behind. Nobody could ask which
of last week's findings rested on the function that just changed.

`said` records one. `standing` asks whether they still hold.

```
gmr read <key> --json          → fact_address, per anchor
gmr said "<what you found>" --on <key> --saw <address> --depends '<invariant>'
gmr standing                   → still holds · no longer holds · never looked
```

The agent carries the address across itself, and that is the point rather than
an awkwardness. `said` could sample the anchors and fill `saw` in — and it would
then be recording what the anchor reads *now*, not what the agent was shown,
which is exactly the lie `shown` exists to catch. A conclusion that cites a
reading nobody took comes back `unseen`, and `said` warns at write time rather
than refusing: the record of what was believed is worth keeping even when it was
believed badly.

## What `depends` buys here that `Holding` does not

`Holding` compares the whole state, so any change to a watched coordinate reports
moved. `depends` lets the author say **which part they relied on**. A finding
about a function's signature survives an edit to its body:

```
crates/gmr-runtime/src/open.rs#blind   the ground moved: now.body · v.logic
depends: still holds
```

That is the structure of dependence, stated by the one party that knows it and
checked by a party that cannot read the sentence — [[gmr-not-entailment]]'s line,
paying for itself.

An invariant over `v.*` is worth less than it looks: those bits are sticky by
construction, so once one fires the invariant stays broken until somebody
re-baselines. That is right for the memory loop, where a note stays due until a
person accepts it, and it means a conclusion whose ground moved is not un-made by
reverting the change. You re-conclude; you do not un-conclude.

## Three counts, because they are three different things

`standing` ends with how many the ground no longer settles, how many were built
beside an anchor rather than through it, and how many cited no reading at all.
Collapsing the last two is what makes an anchor decorative. Only the first two
set the exit code: citing nothing is an absence, not a defect.

**Who decides whether a moved ground reaches a conclusion is the author.**

```
depends broken / unevaluable   due          its own stated condition failed
depends vacuous                due          what was written could not have failed
depends holds                  fine         the author said it survives this
depends unstated + ground moved  due        nobody said it survives
depends unstated + ground still  fine
```

`vacuous` sits with `broken` rather than with `unstated`, and it does not wait
on the ground having moved: an invariant the world cannot reach is a green light
earned by saying nothing while appearing to say something, which is worse than
saying nothing plainly.

The third line is the one that had to be added. Counting only `Broken` meant a
conclusion that vouched for nothing exited zero however far its ground had
moved — a green light earned by saying nothing, which is the exact answer
`Depends::Unstated` exists to refuse one field down, arriving again in the exit
code.

`Holds` beating a moved ground is not a hole in the same shape: the author
*wrote something down*, and it said this move is survivable. Overruling them
would make `depends` decoration. What GMR cannot know is whether the conclusion
is still **true** once the thing it described is fixed — that is entailment, and
retiring it is the job of whoever fixed it.

## Retiring, because an append-only log still needs a horizon

`standing --retire` revokes the binding. What the conclusion said stays in the
table — it is evidence of what was believed and nothing deletes that — and
nothing asks about it again. Without it the verb accumulates every conclusion
ever made, forever, which is the shape in which a report stops being read.

`Runtime::claims` is what makes the argument-free form possible: `ground` answers
about claims a caller already holds, and until this there was no way to ask which
claims exist. `standing` filters to the ones bound to nothing, which is how a
retired conclusion leaves the list without leaving the journal.

## When this changes, ask

Does `said` start filling in `saw` for the caller? Then every conclusion cites
the reading the anchor is on rather than the one its author saw, `shown` reads
`seen` for all of them, and the check has been turned into a decoration that
always passes.

Does `standing` start marking a conclusion `broken` and handing it to a person to
re-read? That is the memory loop's processing applied to an inference, and the
`--why` seal exists because the two are not the same act.

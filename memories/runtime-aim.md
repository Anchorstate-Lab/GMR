---
about:
  - crates/gmr-runtime/src/health.rs#Aim
  - crates/gmr-runtime/src/health.rs#aimed
  - crates/gmr-runtime/tests/operations.rs#an_anchor_reports_whether_its_firing_ever_changed_a_memory
  - console/cli/src/verbs/health.rs#run
watch: [sig, logic]
---

# Whether an anchor is pointed the right way, measured rather than assumed

`docs/ARCHITECTURE.md` §1.3 closes on its own first risk: the guarantees are
conditional on the declaration, and **whether the author declared the right
directions decides whether the system is worth anything**. Everything else here measured whether
memories drift. Nothing measured that.

Two failure modes, and each has a number now:

```
never fired            readings > 0, answered = 0
                       a direction nothing moves in -- or a fact that settles the
                       judgement by itself, which the anchoring heuristic says not to anchor
fired, changed nothing answered > 0, moved_a_memory = 0
                       it comes back, a person re-reads, and the note never needs
                       a word changed
```

`moved_a_memory` is the one worth having. It is **precision**, in the alerting
sense: of the times this anchor handed something back and a person answered, how
often did the answer involve rewriting what was handed back. Alert quality
normally needs labelled outcomes and a postmortem to get them; here both halves
are already durable and neither was being read. A `Restate` revision is in the
journal at a known seq, and a memory's `bound_version` moving is in the bindings
table against a `bound_at_seq` on the same counter. So: between one restate and
the next, did any claim on this anchor get a new version.

## One confirmation is not a false alarm

Handing a memory back and having its author decide it still holds is the system
doing its job — that is what `accept --why` seals. A **run** of them is the
signal, and only a person can decide where the line is, so this reports rates and
never a verdict. It stays in `health` and out of `check`'s exit code: a
badly-aimed anchor is a criteria judgement, and criteria are the owner's
([[constitution]]).

## What it said the first time it ran here

652 anchors, 2643 hand-backs, 655 of them answered by rewriting a memory — **25%**
— with 110 anchors that have never fired and 97 that have fired and never once
changed a note. The last group is where a `watch:` is worth narrowing.

Read those honestly: a bulk `accept --baseline` over a hundred anchors in one
sitting lands in the denominator, and most of those notes genuinely did not need
a word changed. That is not the metric being wrong; it is the metric describing
what happened.

## When this changes, ask

Does `aim` grow a threshold, a grade, or an exit code? Then this layer is
deciding what a good anchor is, which is the domain's and the owner's
([[gmr-not-entailment]] draws the same line one axis over). Report the rate;
somebody who knows the corpus reads it.

Does `moved_a_memory` start counting a version stamp that no rewrite caused —
`reaffirm` re-stamping the same bytes, a provider that versions by fetch time?
Then precision inflates toward one and the number stops meaning anything. It
counts a **changed** version, per claim, which is why it compares rather than
counting rows.

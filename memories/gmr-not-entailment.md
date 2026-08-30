---
about:
  - "docs/GMR.md#GMR 架构 > 0. 是什么"
---

# The line between structure and entailment is what makes this auditable

The comparison that produced this section was with the citation and attribution
work every retrieving assistant now does — mark the source, and in the careful
versions check that the output is actually entailed by it. That field answers
**was there ground for this sentence when it was said**. It is a one-shot
question and it is answered at generation time.

GMR answers a different one: **is that ground still there, and still that
shape**. Continuous, and re-asked every time the world moves. `saw` / `shown`
([[runtime-ground]]) answer a third: **was the sentence built from that ground
at all**, which is neither of the first two and lives on the delivery path
rather than in a post-hoc annotation.

None of the three substitutes for another. A correctly cited sentence is still
wrong once the source is rewritten; a perfectly anchored sentence whose author
never looked at the anchor is anchored decoration.

## What this system will not do

It says a claim is bound to these anchors; that those anchors read differently
now than when it was bound; that it cited a reading this anchor never took;
that the invariant its author wrote down no longer holds. It does **not** say
the claim is therefore false.

That is not modesty. Structure is something a third party can recompute from the
journal, the store and the probe, byte for byte. Entailment is not — anything
that decides "this sentence no longer follows" is a judge nobody can audit, and
building one into the base would put the one unfalsifiable component in the
place everything else rests on.

The place for that judgement is the caller's, with the structure in front of it.
`Depends` is the closest the base comes, and it is deliberately not entailment
either: it evaluates an expression **the asserter wrote down**, over anchor
states, and reports what the expression said. The base supplies no opinion about
whether that was the right expression.

## When this changes, ask

Does anything in `crates/` start comparing a claim's content against a reading?
That is entailment, and it is the boundary this section names. A domain may do
it; the base may not, and `Standing` may not carry its verdict.

Does a verdict field appear that folds `holding`, `shown` and `depends` into one
score? Ask what its precedence rule says — [[runtime-warrant]] records what
happened the last time one was shipped from here.

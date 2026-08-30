---
about:
  - crates/gmr-runtime/src/read.rs#Depends
  - crates/gmr-runtime/src/read.rs#depends
  - crates/gmr-core/src/memory.rs#Binding
  - crates/gmr-expr/src/ast.rs#Quant
  - crates/gmr-expr/src/ast.rs#Over
  - crates/gmr-expr/src/eval.rs#quantified
  - crates/gmr-expr/src/parse.rs#quantified
  - crates/gmr-runtime/tests/grounding.rs#an_invariant_reads_every_anchor_the_claim_names_as_one_question
  - crates/gmr-runtime/tests/grounding.rs#a_claim_that_stated_no_invariant_is_not_reported_as_keeping_one
  - crates/gmr-runtime/tests/grounding.rs#rebinding_with_a_different_invariant_is_a_new_assertion
watch: [sig, logic]
---

# The asserter says what its claim rests on, and true means it still stands

Before this, a claim's standing was assembled by whoever read it: a `Holding` per
anchor, and the reader deciding what combination of those meant the sentence was
still good. That decision was made differently at every call site and written
down nowhere, which is the same defect [[runtime-bound]] describes one layer
down — a fold every caller reimplements, with the type unchanged either way, so
nothing reports the disagreement.

`Binding.depends` is one expression, written by the asserter, and `ground`
answers it: `Holds` · `Broken` · `Unevaluable{why}` · `Unstated`.

## The polarity is inverted from a subscription because the layer is different

The coding domain's `watch:` fires **when something moved**. `depends` is quiet
when everything is fine. That is not an inconsistency waiting to be tidied: they
sit in two different layers ([[three-layers]]).

`watch:` belongs to a **memory** — a long-lived constraint, authored and
reviewed, whose failure mode is drift and whose processing is *a person re-reads
it*. A memory whose axis moved is not false, it is due.

`depends` belongs to an **inference** — one turn's conclusion, whose failure mode
is losing its ground and whose processing needs nobody: the condition its own
author wrote down stopped holding, and there is nothing to re-read.

"Bring this back for judgement" and "this still holds" are not one predicate
wearing two signs, and building either out of the other puts a memory's criteria
in the append-only log or an inference's conclusion in a reviewed file. The first
would have `check` comparing code against a copy of the memory rather than the
memory.

`Unstated` is a variant rather than a `Holds`. A green light earned by saying
nothing is the one answer this field must never give — it would make every claim
that predates the column, and every claim whose author could not be bothered,
indistinguishable from one whose invariant was checked and held.

`Unevaluable{why}` is the third refusal to guess: a body that answers with a
number is not a yes or a no, and reading it as either puts a claim in a bucket
its author never asked for.

## One expression over the whole set, not one per anchor

A sentence resting on four facts is one question, not four. `gmr-expr` gained
quantifiers rather than a wildcard path:

```
all(anchors, not state.v.sig and not state.v.logic)
any(anchors, state.now.value.12.price_cents != 420)
count(anchors, state.v.roll)
```

Inside one, `state` is the anchor being asked about, so every expression that
already worked over one anchor works unchanged over a set — no element-wise
broadcast, no lambda syntax, one new node. The alternative, `anchors.*.<path>`
producing a list, needs comparison to broadcast over that list, and broadcast is
a rule nobody can see at the point where it fires.

An empty set keeps `all` and breaks `any`, which is what an invariant over
nothing has to mean: a claim bound to no anchor has nothing that could have
broken it, and reporting `Broken` would file every unbound claim beside the ones
whose ground moved.

An anchor missing the field abstains rather than faulting, because state reads
lenient — one anchor with nothing to say does not take the whole invariant dark.

## What it cannot express, and why that was not fixed

There is no way to say "the element of this list whose `id` is 11". Selecting by
predicate needs a lambda, and the evaluator is deliberately small and pure
([[eval-version]] earns a version over the whole of it). So an invariant over a
reading that is a *list of things* has to index — `state.now.value.12` — and an
index is only a name while the order holds.

That is a real limit and it decided a real case: the restaurant channel built on
this SDK writes no `depends` at all, because `saw` and `Holding` already answer
its question and the only invariant it would have wanted is index-shaped. The
shape `depends` fits is the one the coding domain has — booleans at named paths.
Reaching for a lambda before there are two callers that need one would be
abstraction ahead of evidence.

## `depends` is part of what an assertion says

`Bound::says` compares it, so rebinding the same claim to the same anchors with a
different invariant writes a row. Treating that as a repeat would leave the table
holding one sentence and the reader reading another. `reaffirm` carries the
standing one forward, the same way it carries `saw`.

It lives on `Binding` and not on `Asserted` because it is a property of the
relation, not of the occasion: [[memory-Binding]]'s test is whether the field
should stay unchanged when the binding is replayed, and this one should.

## When this changes, ask

Does anything start deriving `depends` for a claim that did not write one —
from its anchors' shapes, from a default? Then `Holds` stops meaning "its author
said what it rested on and that still holds" and starts meaning "we guessed",
and the two are indistinguishable in the output.

Does a caller fold `depends` together with `holding` and `shown` into one
verdict? That is the reduction [[runtime-warrant]] records the last shipping of,
and [[gmr-not-entailment]] says why the base does not do it.

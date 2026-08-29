---
about:
  - crates/gmr-runtime/src/read.rs#Instructions
  - crates/gmr-runtime/src/read.rs#refresh
  - crates/gmr-runtime/src/read.rs#grounded_within
  - crates/gmr-runtime/src/observe.rs#observe_within
watch: [sig, logic]
---

# GMR takes observation instructions and never takes a policy

The line this type draws: **a parameter belongs in GMR when it changes what GMR
does, not when it is a kind of judgement.** `Budget` was already on the right
side of it — the caller hands one over, it decides whether to keep probing, and
running out shows up as an answer's state rather than as a verdict. `Instructions`
is the same species and says so by sitting next to it:

```
max_staleness   re-probe, or serve what is on record        changes what we do
budget          how long this call may spend reaching out   changes what we do
```

Neither is a threshold GMR applies to grade an answer. Whether six hours is too
old is the caller's question and GMR never answers it — it reports `observed_at`
and stops ([[runtime-warrant]]). What a freshness bound decides is only whether
to go and look before answering.

The counter-example is the one that keeps this honest: `on_unreachable: deny`
would not change a thing GMR does. It changes what the caller concludes, so it
belongs to the caller, and GMR would have to be handed a policy to accept it.

## An instruction-free call is a default, not an absence

`grounded(key)` is `grounded_within(key, &Instructions::default())`, and that
default is "serve the reading on record, whatever its age". Saying it that way
matters: the read path had no freshness behaviour to speak of before, and it
would be easy to read the short form as *unpoliced* rather than as *this policy*.

## A refresh that could not happen is said, not inferred

A probe that fails lands an `Attempt` in the log the ordinary way and comes back
on the knowledge axis as `Blind` — that is the whole reason that axis is separate
from what the fact did, and it needs nothing extra here.

A **held lease** is different: nothing failed, nobody looked, and the call
returns. That was swallowed at first, on the argument that another writer is
already doing the looking and the reading carries its own date, so a caller could
tell by comparing `observed_at` against the bound they passed. Both halves are
true and the conclusion was still wrong. Leaving a caller to *infer* that their
instruction was not carried out is a failure path with nothing on it, which is
the one thing CLAUDE.md refuses outright — and it is the product boundary in the
mirror: whether to wait, retry, or accept what is on record is exactly the kind
of judgement GMR does not make, and it cannot be made from an answer that does
not mention the question.

So `Leased` propagates. A busy anchor stops the call rather than quietly
answering a different one.

## Not `Policy.stalled_staleness_secs`

There is a staleness number in `Policy` already and these must not be merged. It
is one global value on the scheduler, it drives an `edges` **report**, and it
never decides whether to go and look. Same word, different layer, opposite job:
one describes a condition to a reader, this one instructs an observation.

## The wire shape, and why it waited for a consumer

`Instructions` had no serde derive at all until there was somebody outside Rust
to hand one in. That was deliberate: `Option<Duration>`'s own serde form is
`{secs, nanos}`, which no caller outside Rust would write and none should have to
read, so the shape is a decision and not a derive — and deciding it with zero
consumers would have been guessing.

It is **milliseconds, as a plain integer, with the unit in the field name**:
`max_staleness_ms`, `budget_ms`. A number cannot carry a unit, and `Policy` had
already settled that spelling for every span it holds; a second convention here
would make one of the two wrong at a glance. A span nobody asked for is absent
rather than null — an instruction says what it wants bounded, and silence is how
it says the rest is unbounded.

`deny_unknown_fields`, on both this and `Policy`. A caller who writes
`maxStaleness` and is quietly ignored gets an answer served from the record under
a freshness bound they believe they set — the failure is invisible from the
outside and looks exactly like a fresh answer. That is the silent path CLAUDE.md
refuses, arriving through the wire instead of through the code.

## When this changes, ask

Does a field arrive that GMR would only use to decide what the caller should
conclude? That is a policy, and the product boundary is that GMR does not take
one. Ask what GMR would *do* differently with it — if the answer is "nothing",
it belongs to the caller.

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

## Asking is not promising

A refresh that cannot happen does not fail the call. A held lease means another
writer is already doing it, so the stored reading is served — and the reading
carries its own date, so nothing is hidden by serving it. A probe that fails
lands an `Attempt` in the log the ordinary way and comes back on the knowledge
axis as `Blind`, which is the whole reason that axis is separate from what the
fact did.

## Not `Policy.stalled_staleness_secs`

There is a staleness number in `Policy` already and these must not be merged. It
is one global value on the scheduler, it drives an `edges` **report**, and it
never decides whether to go and look. Same word, different layer, opposite job:
one describes a condition to a reader, this one instructs an observation.

## When this changes, ask

Does a field arrive that GMR would only use to decide what the caller should
conclude? That is a policy, and the product boundary is that GMR does not take
one. Ask what GMR would *do* differently with it — if the answer is "nothing",
it belongs to the caller.

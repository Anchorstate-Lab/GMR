---
about:
  - crates/gmr-core/src/anchor.rs#Recorded
  - crates/gmr-core/src/probe.rs#digested
  - crates/gmr-runtime/src/observe.rs#observe_into
watch: [sig, logic]
---

# Two knobs, because how much is kept and whether it may be plaintext are two questions

`Recorded` sits **beside** `retain` in [[anchor-RunSettings]], not inside it.
The plan that asked for this asked for a third `Retain` variant, and that would
have made two independent questions exclusive:

```
retain   Tick / Full      when the state did not change, is an entry written at all
facts    Plain / Digests  when one is written, may its facts be plaintext
```

A `Retain::Digest` answers the second and leaves the first with no answer — and
"which question does this variant not answer" is the tell. Every combination is
reachable and means something: keep every observation and let none of them hold
a secret is `Full` + `Digests`.

This is the same shape as [[runtime-warrant]]'s: an enum forces exactly-one-of
on its members, so two things that can both be true at once belong on two axes.

## Refusing is the enforcement

An anchor whose facts are secret cannot be protected by asking its probe nicely.
Declaring the mode is worth nothing unless something mechanical stops the
plaintext, so an undigested reading on a `Digests` anchor is **refused**: the
observation never becomes an `Observation`, so nothing derived from it reaches
the log — not the facts, and not the state the rules would have built from them.

Replacing the facts with their hash on the way in was the other option and it is
weaker in the way that matters. The rules run on the plaintext, and what they
put in `state` is the domain's choice, which the base may not read (rule 11). A
substitution would have left that half standing while looking like a guarantee.

The refusal is `Unusable`, and the sentence [[render-warrant]] prints for that
class names whose problem it is: the probe answered, and its answer cannot be
used here. Not `Unreachable` — the world was fine. Not `Unevaluable` — the rules
never ran.

## The check lives where an `Observation` is made, not where one is written

`open` and `observe_with` both append an observation, and a guard on one of them
is the same "mostly" that `docs/GMR.md` refuses for write tokens: a bypass makes
the guarantee a habit. Both already funnel through `observe_into`, which is the
only way an `Outcome` becomes an `Observation`, so that is where it went — a
third write path cannot skip a question it has to pass an argument to.

## What counts as digested

Every leaf in the facts must be a sha256 hex string. Numbers, booleans and nulls
are refused too, which is stricter than it needs to be for secrecy and easier to
state: under this mode nothing but digests. Object keys are not checked — a key
is the probe's vocabulary, and the base does not read vocabulary.

## When this changes, ask

Does a knob arrive that decides *what* is written rather than *whether*? It is
this axis, not `retain`. Does one decide whether we go and look at all? That is
`cadence_secs`, and the entry test in [[anchor-RunSettings]] still governs: no
field here may be an input to a transition.

---
about:
  - console/cli/src/verbs/sync.rs#check_contract
  - console/cli/src/verbs/sync.rs#hand_written_rules_and_a_named_shape_both_become_transitions
  - console/cli/src/verbs/sync.rs#a_shape_the_probe_cannot_feed_is_refused
  - console/cli/src/verbs/sync.rs#roster_rides_the_same_probe_happily
  - console/cli/src/verbs/sync.rs#hand_written_rules_get_the_same_check_a_shape_gets
watch: [sig, logic]
---

# A named shape and hand-written rules are the same kind of thing once expanded

`to_transitions` turns either a named `shape` or a literal `rules` list
into the same `Transitions` value — the substrate never sees which form
the declaration used, only the resulting rule table. `check_contract`
takes advantage of exactly that: it calls `crate::contract::reads_of` on
whatever `to_transitions` produced, not on some shape-specific declared
list, so a hand-written rule table (what an agent writes by default, with
no shape at all) gets *the identical* obs-field check a named shape gets.
There is no separate, weaker path for hand-written rules — the escape
hatch is that a note may write its own rules, not that doing so skips
validation.

`check_contract` runs at sync time, before any anchor opens and before any
observation — `a_shape_the_probe_cannot_feed_is_refused` pins down that a
rule reading an obs field the declared probe does not emit is caught here,
not discovered later when `observe` first tries to evaluate it and fails
loudly at runtime instead.

## When this changes, ask

Does a new declaration form (a third way to specify transitions, beyond
`shape` and `rules`) get its own contract-check code path, or does it also
funnel through `to_transitions` before `check_contract` inspects the
result? A separate path could let a new declaration form skip this check
entirely.

## One predicate, one translation, and the answer travels with the artifact

`Runtime::open` refuses an anchor whose rules read a field the probe declares it
never reports. This one asks the same question before opening, so a bad
declaration is refused with the key named rather than as an `open` failure — and
it is the *same* question now, not a second implementation of it.

`Obs::observes` is the one translation from this domain's `{schema, at, facts}`
into the base's `Observes`; `Observes::covers` is the one predicate. `unmet` is a
two-line adapter over both. `known` is gone — it reimplemented `covers` against
a second reading of the same declaration.

And the declaration reaches the transport now: `Obs::observes` goes into the
shell `Manifest`, which is inside `Manifest::address`, so what the probe claims
to report cannot change without the version it is addressed by changing. It used
to sit in `.anchor/probes.toml` where the transport never saw it, which is why
shell probes answered `Observes::Unknown` for exactly the probes where somebody
had already written the answer down.

What is still not checked is whether the declaration is **true** of the program.
`open` warns when the first observation reports fields the declaration never
mentions ([[probe-Derivation]]); the opposite direction — declaring a field the
program never prints — surfaces as `NoSuchField` when a rule reads it, which is
late but not silent.

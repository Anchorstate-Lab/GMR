---
about:
  - domains/coding/cli/src/verbs/sync.rs#check_contract
  - domains/coding/cli/src/verbs/sync.rs#hand_written_rules_and_a_named_shape_both_become_transitions
  - domains/coding/cli/src/verbs/sync.rs#a_shape_the_probe_cannot_feed_is_refused
  - domains/coding/cli/src/verbs/sync.rs#roster_rides_the_same_probe_happily
  - domains/coding/cli/src/verbs/sync.rs#hand_written_rules_get_the_same_check_a_shape_gets
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

## This check now has a twin, and they read two different sources

`Runtime::open` refuses an anchor whose rules read a field the probe declares it
never reports, from `Derivation.observes` ([[probe-Derivation]]). This one asks
the same question of `.anchor/probes.toml`'s hand-written `obs` — the second copy
that `observes` was added to delete, and which was only deleted for the four
built-in extractors. For a shell probe, that hand-written list is still the CLI's
authority, and nothing compares it against what the program prints.

Two implementations of one check, on two sources that can disagree. It is
recorded as a live conclusion here rather than only in prose:
`gmr standing said:two-unmet-implementations` names both functions and comes back
when either moves.

The fix is for a shell recipe to carry its `obs` into the transport as
`Observes::Named`, at which point this function asks the runtime instead of the
recipe file. Until then, the declaration is reviewed and the program is not.

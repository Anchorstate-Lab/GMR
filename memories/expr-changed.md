---
about:
  - crates/gmr-expr/src/eval.rs#changed
  - crates/gmr-expr/src/eval.rs#a_direction_the_probe_does_not_report_is_a_typo_not_a_verdict
  - crates/gmr-expr/src/eval.rs#changed_faults_exactly_where_the_same_path_would
  - crates/gmr-expr/src/eval.rs#a_world_that_is_not_there_is_still_an_answer_not_a_typo
  - crates/gmr-expr/src/eval.rs#the_state_side_stays_lenient
watch: [sig, logic]
---

# `changed(x)` is sugar for `obs.x`, so it inherits obs's strictness and state's leniency

`changed` reads the same two sides `obs.x` would, and has to fault exactly
where `obs.x` would: on the `obs` side, a name the probe's object does not
have is `NoSuchField` — a typo, not a verdict — because staying silent would
mean `bind` catches the typo as a path expression but `changed()` on the
same field would silently say "false". Two ways of asking about the same
fact must not disagree about whether it is even askable.

The `state` side is deliberately lenient: the first time a direction is
observed, `ctx.state.get(name)` has nothing to compare against, and that
absence is not an error — it is what "the domain has never seen this
direction before" looks like. There is one further asymmetry, not a
special case: when `obs` itself is `Value::Null` (a `NotFound` probe
result), that is the world's answer, not our mistake — `changed` reports a
real transition (`true`) if the state previously had a value, and `false`
only if the state never had one either.

This obs-strict/state-lenient split, and treating `changed()` as reading the
same fact `obs.x` reads, are semantic decisions of the anchoring layer, not
generic evaluator behavior — see the crate-boundary note that `gmr-expr`
has no compile-time dependency on `gmr-core` but still has to honor this
convention.

## When this changes, ask

Does the new behavior make `changed("x")` disagree with `obs.x` about
whether `x` is askable, or make it forget that a `state` absence on first
sight is not an error? Either one reopens exactly the inconsistency this
function exists to close.

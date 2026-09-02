---
about:
  - console/cli/src/verbs/observe.rs#run
  - console/cli/src/verbs/check.rs#run
watch: [sig]
---

# `observe`'s exit code answers a different question than `check`'s, and that difference is load-bearing

Both walk the same anchors and both call `rt.observe`, which made a CLI-surface pass
consider `observe` dead weight next to `check` — `check` calls into
`observe.rs#delivered`/`observe.rs#report_unclaimed` already, so the two verbs share
their delivery logic. Their **exit codes** do not share a meaning, and that is not an
oversight to fix by pointing one verb's callers at the other.

`observe.rs#run` exits 1 whenever `moved > 0` — `moved` counts every
`Observed::Transitioned`, full stop. It does not ask whether any memory was bound to
that anchor, let alone whether a subscription's `watch:` cared about the axis that
moved.

`Observed::Contended` prints and does not count. Another writer recorded first and
nothing was written, so there is no transition to report and no reason to fail a
run: the anchor is exactly as observed or unobserved as it was before.

`check.rs#run` exits 1 for `handed` (a memory was actually delivered) or `unclaimed`
(something moved with no memory bound at all) or one of the criteria/instrument
diagnoses ([[check-drift]]) — but explicitly *not* for `quiet`: an anchor that moved on
an axis no memory's `watch:` names is deliberately reported at exit 0, as "N anchors
moved on axes nobody asked about — `gmr status` shows them." That distinction is the
entire reason `check` computes `moved`/`quiet` separately instead of just counting
transitions.

That exit-0 branch is real; the sentence quoting it is not always printed.
`check`'s closing `match (handed.len(), quiet)` makes the two arms exclusive,
so when a memory was handed back *and* something moved unwatched, only the
first line prints and `quiet` is dropped from the human output. `--json` still
carries it as `moved_unwatched`, which is the reliable reading. The exit code —
the thing this memory is about — is unaffected either way.

So `observe`'s signal is "did the state machine move at all" (every axis, every
anchor, subscribed or not); `check`'s is "does a human need to look" (filtered through
what memories actually watch). A script polling `gmr observe`'s exit code and a script
polling `gmr check`'s exit code can disagree on the same repository state, and both
are answering the question they were built to answer.

## When this changes, ask

Converging these into one exit-code meaning is a change to what counts as a
reportable movement — CLAUDE.md §7's "criteria: probe, rules, terminal, state revision
semantics" is exactly this kind of call, and it needs the owner's decision plus an
announced breaking change (any script keyed to `observe`'s current "any transition"
exit code silently starts seeing fewer failures, or `check` silently starts seeing
more). It cannot be done as a side effect of collapsing CLI surface.

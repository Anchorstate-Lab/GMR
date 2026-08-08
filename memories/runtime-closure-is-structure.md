---
about:
  - crates/gmr-runtime/tests/state_machine.rs#restate_cannot_resurrect_a_finished_anchor
  - crates/gmr-runtime/tests/state_machine.rs#emptying_the_terminal_set_cannot_resurrect_a_finished_anchor
  - crates/gmr-runtime/tests/state_machine.rs#a_terminal_transition_is_remembered_even_after_the_state_moves_on
watch: [logic]
---

# Closure is a structural fact in the fold, not the current interpretation of the state

If "is this anchor closed" were computed only from the *final* state after
folding the whole log, closure would be a view rather than a fact — and
any later action that moves the state back out of what currently looks
like the terminal set would silently resurrect an anchor that had already
finished. `fold` instead makes closure sticky: once any entry in the log
put the anchor into its terminal set, `closed` stays `true` for every
later fold, regardless of what a subsequent `Entry::Revise` does to
`state`.

`a_terminal_transition_is_remembered_even_after_the_state_moves_on` proves
this by constructing a raw journal by hand — `Open` → `Transition` into
`settled` → `Revise` restating `status` back to `pending` — specifically
to verify the stickiness lives in `fold` itself, not in some guard inside
`revise` that happens to reject the write. `restate_cannot_resurrect_a_
finished_anchor` and `emptying_the_terminal_set_cannot_resurrect_a_
finished_anchor` are the two ways someone might try to route around
closure from the live API: restating the state, or revising the terminal
set itself so the current state no longer matches it. Both are refused —
correcting a wrong criterion requires a new generation via `supersedes`
(see [[runtime-open-supersede]]), never resurrecting the old one.

## When this changes, ask

Is `closed` still derived by folding the entire entry history, or does
some path compute it from only the latest state and the *current*
`terminal` set? The latter reopens the exact resurrection bug this
structure exists to prevent.

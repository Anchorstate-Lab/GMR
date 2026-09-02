---
about:
  - console/cli/src/verbs/revise.rs#run
  - console/cli/src/verbs/revise.rs#choose
watch: [sig]
---

# One shell for four hand-written `Change` variants, and `--state` is not the fourth declarable facet

`reprobe`/`retransition`/`reterminal`/`restate` used to be four separate verbs, and
each was the same shape: resolve the key, build one `gmr::Change` variant, `rt.revise`,
print. That shell is what got merged — `choose` picks exactly one of
`--probe`/`--rule`/`--terminal`/`--state` the same way `accept.rs#choose` picks exactly
one of `--baseline`/`--criteria`, and each branch below it still does what its own
verb used to do, output format included (`--probe` still warns about
`incomparable_state`, `--terminal` still says when the anchor just closed, `--state`
still prints the before/after diff).

**`--state` is not a fourth thing `sync::differs` could ever compare.** `differs`
(`sync.rs`) checks exactly three facets — probe, rules, terminal — because
`AnchorDecl` only has fields for those three; there is no declared target state to diff
a live anchor's state against, and `accept --criteria` (see [[check-drift]]) only ever
synthesizes `Reprobe`/`Retransition`/`Reterminal` from that diff, never `Restate`. So
if `--probe`/`--rule`/`--terminal` ever grow a declaration-driven path — pass a decl in
instead of raw flags, the way `accept --criteria` already does — `--state` is not next
in that line; it has nothing to diff against and stays the always-manual branch.

**This is not the recapture pattern [[verbs-recapture]] warns about.** That note's
warning is about a *fourth place* assembling `Restate { state: { position } }` (clearing
state back to bare position and letting the shape's capture rule run again) instead of
calling the one `recapture` helper in `mod.rs`. `revise --state` does something
unrelated: it takes the caller's verbatim JSON as the new state, no clearing, no
capture rule involved. Merging its CLI shell into `revise` did not touch that helper or
add a new caller of it.

## When this changes, ask

Does `--probe`/`--rule`/`--terminal` gain a way to take a value from a declaration
(mirroring `accept --criteria`) rather than always being hand-typed? If so, does
`--state` get left as the one branch that stays manual on purpose, or does someone
try to give `AnchorDecl` a state facet — the latter is `sync::differs`'s whole three-facet
design being re-opened, not a `revise` change, and needs the owner's call.

---
about:
  - crates/gmr-runtime/src/edges.rs#walk
  - crates/gmr-runtime/tests/operations.rs#the_world_being_out_of_reach_still_waits_for_the_streak
watch: [sig, logic]
---

# `walk` rides `scan`'s one projection; it does not rebuild state a second way

`walk` calls `gmr_core::scan` (see [[journal-scan]]) rather than
re-deriving "is this closed", "did the state change" from the entries
itself. Edges are the only thing a consumer of `changed_since` ever sees,
so if a hand-rolled second read of the log disagreed with `scan`'s fold
about what counts as closed or changed, nothing downstream would notice —
each read stays internally consistent, and the disagreement only shows up
if someone feeds the same log to both. Riding the one projection instead
removes the chance for that drift to exist.

`the_world_being_out_of_reach_still_waits_for_the_streak` is what pins
down the other side of that same threshold: `ReasonClass::Unreachable` is
about the world, not a rule bug, so it is worth retrying and must not
become a `Stalled` edge on the very first failure — only `Unevaluable`, or
crossing `policy.stalled_attempts`, does that.

Two distinctions inside the callback are load-bearing, not cosmetic.
`Edge::Closed.self_sealed` is `true` unless the entry is `Entry::Close` —
that is the difference between an anchor walking into its own terminal set
(self-sealed) and a human closing it (`Entry::Close`), and the two are
handled differently downstream. And a `Stalled` edge fires only once per
threshold crossing (`ReasonClass::Unevaluable`, or hitting exactly
`policy.stalled_attempts`) rather than on every retry after that point: a
broken rule does not get better on retry ten thousand, so it has to be
loud the first time it is knowable as broken, and it must not share a
counter with "the world is temporarily out of reach."

## When this changes, ask

Does the new logic read `entries` directly instead of going through
`scan`'s callback? That reopens the two-projections drift this function
exists to avoid. And does a stalled/closed condition still fire exactly
once per crossing, rather than repeating on every subsequent poll?

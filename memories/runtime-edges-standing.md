---
about:
  - crates/gmr-runtime/src/edges.rs#Standing
  - crates/gmr-runtime/src/edges.rs#Edges
  - crates/gmr-runtime/tests/operations.rs#an_event_is_handed_over_once_a_condition_is_reported_every_time
  - domains/coding/cli/src/verbs/edges.rs#run
watch: [sig, logic]
---

# `Standing` holds conditions, not events, and dedupes differently because of it

`Edge` reports something that happened in the log after a cursor — handing
one out once is correct because the log entry it came from exists exactly
once. `Standing` cannot work that way: staleness compares the current
clock against the last sighting, and a rewrite asks a content provider
what version it holds *right now*. Neither answer comes from the log, so
there is no cursor position that means "I have already told you this."
Forcing them into `Edge` would re-report the same condition on every poll
with no way for the consumer to tell "new" from "still true" — so they get
their own field, deduplicated by content instead.

`Edges.standing` being `None` is not the same as an empty `Vec`: `None`
means standing was never computed at all (the caller passed a `status`
filter and only wanted matching transitions), while `Some(vec![])` means it
was computed and nothing is currently stale or rewritten.

`domains/coding/cli/src/verbs/edges.rs#run` is where this reaches the
terminal: it prints edges and standing conditions in separate sections
("Current standing conditions (cursor-independent; repeated every time)"),
and prints a distinct message when `out.standing` is `None` versus an
empty `Some`, exactly preserving the distinction this memory describes.

## When this changes, ask

Does the new condition come from the log at a specific `seq`, or from
comparing against the current moment / an external system's current state?
Only the latter belongs in `Standing`; the former is an `Edge`.

---
about:
  - domains/coding/cli/src/verbs/doctor.rs#versioning_is_broken
  - domains/coding/cli/src/verbs/doctor.rs#run
watch: [logic]
---

# `doctor` has one definition per fact, and a strict line between "broken" and "worth noting"

`versioning_is_broken` checks for `.git` because git is how notes are
versioned here — outside a git repository, `bind` still succeeds (it can
still stamp a content hash), but fetching a note back at the exact version
it was bound at cannot work, since that requires history git alone
provides here.

`run` reuses `corpus_health`'s `barren_anchors` for the `barren` list
rather than running a second `memories.is_empty()` scan over the same
`AnchorView`s doctor already has in hand — one definition of "unbound"
instead of two that could quietly drift apart from each other.

The exit code is not "anything at all worth mentioning" — it is
specifically `stranded`, `provider_warnings`, `malformed` notes, or
`undeclared` (see [[check-drift]]): those four mean something declared or
expected is not actually working. `absent`/`barren`/`unseen` are
informational (exit 0) because they can be entirely normal states
(criteria written before implementation, a probe temporarily failing)
rather than something misconfigured.

`undeclared` is computed by `doctor.rs#undeclared`, a second walk over the
same `decls` `check.rs#criteria` builds for its own `undeclared` report —
duplicated because doctor already has `live: &[AnchorView]` in hand and
computing over that is cheaper than the async re-read per key `criteria`
does. The one fact both walks must not diverge on is "is this key named by
a blocking fault" (an `unrouted` `about:` is `unreadable`, not
`undeclared` — the note is right there, it just failed to route). That
fact is not re-derived twice: both call `Scanned::blocked_key`, so a note
that fails to route cannot be misreported as one that was never written.

## When this changes, ask

Does a new signal belong on the "exit 1" side (something is broken) or the
"exit 0, just worth noting" side (a normal, expected state)? And does a
new count reuse an existing source of truth (like `corpus_health`) instead
of re-deriving the same fact a second way?

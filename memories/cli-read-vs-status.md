---
about:
  - domains/coding/cli/src/verbs/read.rs#run
  - domains/coding/cli/src/verbs/status.rs#run
watch: [sig]
---

# `read` and `status` show the same anchors for two different reasons, not the same data twice

They look redundant from the outside — both take an optional key and print anchors —
and a pass at collapsing the CLI's twenty-odd verbs into a front door once considered
folding `read` into `status` as a strict subset. It isn't one. Two things keep them
apart:

**`read` does not filter by `closed`; `status` does, except by name.** `read.rs#run`
prints whatever `rt.read`/`rt.read_all` returns, unfiltered — a closed anchor's last
state is exactly as visible as a live one's. `status` exists to answer "what am I
watching now" (its own doc comment in `cli.rs`), so its whole-repository listing
excludes `closed` — a closed anchor isn't being watched. Naming a specific key is a
different question ("show me this one") and `status <key>` no longer filters `closed`
for that case either, but the no-argument listing still does, on purpose.

**Their JSON shapes are not the same schema at two verbosity levels.** `read --json`
serializes `gmr::AnchorView` verbatim — `attempts`, `sighting`, `derivation`, and per
memory `grounded`/`rewritten`/`retrievable`/`content_at_bind`. `status --json` hand-
builds a projection (`anchor`, `shape`, `status`, `state`, `memories` with an
`unwritten` flag) and adds `criteria_drifted`/`criteria_unreadable`/`criteria_undeclared`
from `sync::audit` (see [[check-drift]]), which `read` never computes at all. Nobody
reading `status --json` should have to know `AnchorView`'s field names, and nobody
debugging a stuck probe via `read` should have to wade through a criteria audit to get
`sightings`/`derivation`. Making one absorb the other means one of those two audiences
starts getting fields it didn't ask for.

So `read` stays: it is the raw substrate dump — the one place the CLI shows exactly
what `gmr-runtime` recorded, closed anchors included, with no curation layered on top.

## When this changes, ask

Does `status --json` grow to carry `attempts`/`sighting`/`derivation`, or any other
`AnchorView` field it currently omits? If it ends up a superset of what `read` prints
(closed anchors included), the schema argument above is gone and folding `read` into
`status --key` becomes a real option instead of a loss of capability.

## Both now take the repository root, to name memories the way their author does

`read` prints the memories bound to an anchor, and it used to print each
one's address. It takes `root` so it can ask the declaring source what that
record is called — a name where there is one, the address where there is
not. `status` already had `root` for its own reasons.

The address is not a name. It reads as one only for a store that happens to
address records by their path, and this repository is that store, which is
exactly why nothing noticed. See [[cli-notes-source]].

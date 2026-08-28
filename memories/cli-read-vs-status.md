---
about:
  - domains/coding/cli/src/verbs/read.rs#run
  - domains/coding/cli/src/verbs/status.rs#run
  - crates/gmr-runtime/src/read.rs#AnchorView
  - crates/gmr-runtime/src/read.rs#MemoryView
watch: [sig]
---

# `read` and `status` show the same anchors for two different reasons, not the same data twice

They look redundant from the outside — both take an optional key and print anchors —
and a pass at collapsing the CLI's twenty-odd verbs into a front door once considered
folding `read` into `status` as a strict subset. It isn't one. Two things keep them
apart:

**`read` does not filter by `closed`; `status` does, except by name.** `read.rs#run`
prints whatever `rt.grounded`/`rt.grounded_all` returns, unfiltered — a closed anchor's last
state is exactly as visible as a live one's. `status` exists to answer "what am I
watching now" (its own doc comment in `cli.rs`), so its whole-repository listing
excludes `closed` — a closed anchor isn't being watched. Naming a specific key is a
different question ("show me this one") and `status <key>` no longer filters `closed`
for that case either, but the no-argument listing still does, on purpose.

**Their JSON shapes are not the same schema at two verbosity levels.** `read --json`
serializes `gmr::AnchorView` verbatim — `sighting`, `faltering`, `derivation`, `facts`,
and per memory `grounded`, `warrant` and `grounding`. `status --json` hand-
builds a projection (`anchor`, `shape`, `status`, `state`, `memories` with an
`unwritten` flag) and adds `criteria_drifted`/`criteria_unreadable`/`criteria_undeclared`
from `sync::audit` (see [[check-drift]]), which `read` never computes at all. Nobody
reading `status --json` should have to know `AnchorView`'s field names, and nobody
debugging a stuck probe via `read` should have to wade through a criteria audit to get
`sightings`/`derivation`. Making one absorb the other means one of those two audiences
starts getting fields it didn't ask for.

So `read` stays: it is the raw substrate dump — the one place the CLI shows exactly
what `gmr-runtime` recorded, closed anchors included, with no curation layered on top.

## Which is why `AnchorView` and `MemoryView` are anchored here

"Serializes it verbatim" makes this note's claim a claim about *those two types*, and
for a while it was anchored only to the two verbs — which do not change when a field
is renamed under them. The list above said `attempts`, `retrievable` and
`content_at_bind` long after `faltering`, `grounding` and `warrant` replaced them, and
`check` stayed green the whole time, correctly: nothing it was watching had moved.
A note that quotes another layer's field names has to watch that layer, or it is
grounded to the wrong thing and the green means nothing.

## `--fresher-than-secs` is the one thing it does besides dump

`read` is also where the freshness instruction ([[runtime-instructions]]) reaches
the CLI, and that makes it the one verb here that can go and look before it
prints. It stays a dump: the flag decides *which* reading is dumped — the one on
record, or one taken just now — and never edits what is printed about it. Unset,
nothing goes out, which is why the rest of this note still describes the whole
behaviour.

It belongs on `read` rather than on `status` for the same reason the two are
separate at all. `status` answers "what am I watching", a question about the
corpus; `read` answers "what does the substrate hold about this", and "hold it as
of when" is the second half of that question.

## When this changes, ask

Does `status` grow a freshness flag of its own? Then two verbs can write to the
journal and the "status only reads" expectation is gone; the argument above says
where the flag belongs, not that it may be in two places.

Does `status --json` grow to carry `attempts`/`sighting`/`derivation`, or any other
`AnchorView` field it currently omits? If it ends up a superset of what `read` prints
(closed anchors included), the schema argument above is gone and folding `read` into
`status --key` becomes a real option instead of a loss of capability.

## Neither one builds the book it names memories from

Both print the memories bound to an anchor, by name. The address is not a name —
it reads as one only for a store that happens to address records by their path,
and a repository whose notes are files is that store, which is why printing the
address instead reads as fine right up until a second store appears. See
[[cli-notes-source]].

`read` takes a `Names` handed down from assembly; `status` keeps `root` for its
own reasons — reading a note's body to decide `unwritten`, and loading the
catalog for the criteria audit — and not for naming. See [[cli-main-run]] for
where the book comes from and why it is not a thing a verb may mint.

`read --json` still prints addresses, not names: [[cli-notes-source]] draws that
line, and an agent handing a string back to `gmr bind` needs the address.

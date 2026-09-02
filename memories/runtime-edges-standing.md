---
about:
  - crates/gmr-runtime/src/edges.rs#Raised
  - crates/gmr-runtime/src/edges.rs#Edges
  - crates/gmr-runtime/tests/operations.rs#an_event_is_handed_over_once_a_condition_is_reported_every_time
  - console/cli/src/verbs/edges.rs#run
watch: [sig, logic]
---

# `Raised` holds conditions, not events, and dedupes differently because of it

`Edge` reports something that happened in the log after a cursor — handing
one out once is correct because the log entry it came from exists exactly
once. `Raised` cannot work that way: staleness compares the current
clock against the last sighting, and a rewrite asks a content provider
what version it holds *right now*. Neither answer comes from the log, so
there is no cursor position that means "I have already told you this."
Forcing them into `Edge` would re-report the same condition on every poll
with no way for the consumer to tell "new" from "still true" — so they get
their own field, deduplicated by content instead.

`Edges.raised` being `None` is not the same as an empty `Vec`: `None`
means standing was never computed at all (the caller passed a `status`
filter and only wanted matching transitions), while `Some(vec![])` means it
was computed and nothing is currently stale or rewritten.

## Not knowing is itself a standing condition

A provider that will not answer used to produce nothing here at all: the
walk only emitted `Rewritten`, and a failed fetch left `rewritten` false,
so a store being down made `gmr edges` report that everything was fine.
`Gone`, `NoProvider` and `Unreachable` are now their own variants for the
same reason `Rewritten` is one — each is true *right now* and none of them
comes from the log.

They stay separate rather than collapsing into one "could not check"
because they are not the same person's problem, and that decides whether CI
should go red; [[runtime-grounding]] carries that split.

`console/cli/src/verbs/edges.rs#run` is where this reaches the
terminal: it prints edges and standing conditions in separate sections
("Current standing conditions (cursor-independent; repeated every time)"),
and prints a distinct message when `out.raised` is `None` versus an
empty `Some`, exactly preserving the distinction this memory describes.

## `since` only reports what has landed, and landing needs somebody looking

The cursor reads the journal. A transition reaches the journal only when
somebody **observes**, so a deployment that only ever calls `since` sees
nothing move, forever, and looks entirely healthy — there is no failure signal
on this path at all, which makes it the easiest thing here to get wrong.

Two doors, and a deployment must open one:

- `ground(asked, { max_staleness_ms })` observes the anchors it touches on the
  read path. Anchors nobody reads never update.
- a periodic `pass` observes on a cadence, and needs a process actually running
  it.

[[node-sdk]] exposes `since` and deliberately not `pass`: scheduling belongs to
whichever process runs the loop, and one per caller is contention over leases
plus duplicated probe calls. The cost of that choice is that whoever assembles
has to answer "who is observing", and the interface never asks. `docs/ARCHITECTURE.md` §6.4 says the same thing where a deployment is being
planned rather than read about.

## When this changes, ask

Does the new condition come from the log at a specific `seq`, or from
comparing against the current moment / an external system's current state?
Only the latter belongs in `Raised`; the former is an `Edge`.

## The name it used to have

This was `Standing` until the word had to go somewhere else. Three types here
were called that, each answering "how is this right now" about a different
thing, and two of them were about to sit on the same SDK surface -- this one
under `since`, and the answer to "does this sentence still stand" under
`ground`. Only the second is the word's literal sense. This one is a list of
findings: every variant is bad news, and the test beside it already said
`edges` *raises* a standing.

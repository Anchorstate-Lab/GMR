---
about:
  - crates/gmr-runtime/src/read.rs#ground
  - crates/gmr-core/src/journal.rs#scan
  - crates/gmr-runtime/src/observe.rs#settle
  - crates/gmr-runtime/tests/operations.rs#an_unchanged_reading_appends_nothing_and_leaves_the_warrant_where_it_was
watch: [sig, logic]
---

# The head is every entry; the move is only the ones that changed the state

`moved_at` is the seq of the latest entry that changed the state, and it used to
be compared against `bound_at_seq` to answer "did the ground move under this
memory". It no longer answers that — see [[runtime-warrant]]: a recapture
changes the state too, so the seq alone reported a move where the diff showed
none. What `moved_at` does now is **gate** that question cheaply: at or before
it, no state change has happened since the bind, so nothing needs folding -- and
since [[runtime-warrant]]'s `holding` reads the journal only past this gate,
what the gate saves is not arithmetic but a whole-log read.

It used to be compared against `head`.

`head` advances on **every** entry. `Attempt` is an entry — it records that we
could not observe at all. So one failed look marked every memory on that anchor
as standing on ground that shifted, when what actually happened is that nobody
could go and look. That is [[layers]]'s line — the world's answer versus our
failure — crossed at the one place a reader acts on it.

`moved_at` is set in exactly the branches that set `entered_at`: `Open`, a
`Transition` whose state differs, and a `Restate` whose state differs. The two
are one event in two units — when the state changed, and where in the log it
changed. A test holds them together, because two fields for one event is two
chances to update only one.

## A transition that restates the same value is not a move either

`should_still` writes a `Transition` when the state **or the fact address**
changed. An address-only change means the probe's answer differed and the rules
judged it the same. That advances `head` and `latest_seq`, and it does not
advance `moved_at`.

This is Salsa's **early cutoff** at the state level: a recomputed input that
produces an unchanged value must not wake its dependents. Without it every
reformatting, every re-hash, every probe upgrade would hand back the whole
corpus to re-read, and the product would be an alert firehose nobody keeps on.

## The cutoff is stronger when the address matches too, and it reaches the warrant

The paragraph above is the case where the address moved and the state did not: an
entry is written, `moved_at` stays. When **both** match, `settle` appends nothing
at all — the `Some(_) => {}` arm — and answers `Still`. Nothing enters the log,
so nothing can wake anyone.

What makes this safe to do is that the two axes [[runtime-warrant]] separates come
apart here as well, and a test holds all three halves together: `holding` stays
where it was, the log head does not move, and `Knowledge::Seen`'s `at` **does**
advance. Looking again at a fact that did not move is not news about the fact; it
is news about us, and it is recorded as a sighting on the scheduler rather than an
entry in the journal.

Collapse the axes and this becomes unsayable. Either every poll appends — the
firehose this section exists to prevent — or freshness stops improving, and a
caller asking for a fact fresher than an hour is told to re-probe something that
was read a second ago. That caller is `Instructions.max_staleness`, so the cutoff
and the freshness bound are the same mechanism read from two ends.

## Why not derive it in the read path

`edges` already tracks the previous state through `scan` to find transitions, and
the read path could have done the same. That would have been a third copy of
"did the state change" — the objection [[journal-reason]] raises about storing
what can be derived, in the mirror: derive it twice and the copies disagree.

**`edges` still has its own.** It is not redundant with this one and cannot be
replaced by it: `edges` emits an `Edge::Transitioned` *per crossing* while
walking, so it needs the comparison at every entry; `moved_at` is the seq of the
latest crossing and nothing else. Two questions, two answers. What was avoided
was a third computation answering the same question as `moved_at` — not the
existence of a per-entry one.

## When this changes, ask

Does something start comparing a binding against `head` again? That is the same
bug with a new name: `head` is the log's cursor, not the world's. And `moved_at`
is the *state's* cursor: it says the state changed, never that it changed away
from what any particular memory was bound to. Deciding from it alone is the same
bug one notch finer.

Does a new `Entry` variant change the state? Then it sets `moved_at` beside
`entered_at`, and the paired test is what fails if it does not.

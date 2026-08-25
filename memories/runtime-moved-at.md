---
about:
  - crates/gmr-runtime/src/read.rs#ground
  - crates/gmr-core/src/journal.rs#scan
watch: [sig, logic]
---

# The head is every entry; the move is only the ones that changed the state

`MemoryView.stale` answers "did the ground move under this memory since it was
bound". It is `bound_at_seq < moved_at`, and it used to be `bound_at_seq < head`.

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
bug with a new name: `head` is the log's cursor, not the world's.

Does a new `Entry` variant change the state? Then it sets `moved_at` beside
`entered_at`, and the paired test is what fails if it does not.

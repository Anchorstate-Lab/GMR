---
about:
  - "docs/GMR.md#GMR 架构 > 6. 记忆层"
---

# Fact, memory and inference are three things, and the mechanism they share is not the layer

Fact is what a probe reads: the code, the menu, the registry. Recomputable, no
author. Memory is a **long-lived constraint on that fact** — written by a person,
reviewed in a commit, and not derivable from the fact, because a constraint you
can derive is one nobody needed to write. Inference is what one analysis
concluded for the task in front of it: one-shot, not yet condensed into anything.

They fail differently, and that is what makes them three:

```
memory     drifts       the code moved; whether what was written still holds is unknown
inference  loses ground the reading it rested on moved, or it never looked at it
```

The processing is different too, and this is the part that is easy to get wrong.
A memory whose coordinate moved is **not false** — it is due. The verdict needs a
person, which is why `check` says "re-read it" and `accept` demands a sealed
`--why`. An inference whose stated condition failed needs nobody: the sentence is
simply no longer supported, and there is nothing to re-read.

## Two loops, and merging them makes one of them lie

```
memory     check  -> handed back -> a person re-reads -> accept --why
inference  ground -> holding / shown / depends -> the caller decides
```

This note exists because the merge was nearly made. `watch:` in a note and
`depends` on a binding are both predicates over anchor state, written in the same
expression language, and it looked obvious that one should be built out of the
other — compile each note's `watch:` into a `depends` at `sync` time and have
`check` read it back from the store.

That is writing a **memory's criteria into the inference slot**. `check`'s whole
job is comparing code against memory; pointed at the store it would compare code
against *a copy of the memory taken at the last sync*. The one drift this system
exists to catch, manufactured in the checker. And the reverse merge — keeping an
inference as if it were memory — promotes a one-shot conclusion into a constraint
nobody reviewed.

The polarities differ for the same reason, and it is not an inconsistency to be
tidied away. `watch:` is true when the memory must come back; `depends` is true
while the claim still stands. "Bring this back for judgement" and "this still
holds" are not the same predicate wearing two signs.

## Where a criterion lives is decided by which layer it is

**A memory's criteria live in the repository**, in the file, read fresh — so
editing `watch:` takes effect on the next `check` with no build step between the
author and the checker.

**An inference's criteria live in the append-only log** — unchangeable, because
they are not a source of truth to be re-read but evidence of what was believed at
the time.

`Source::independent()` already records this axis and always has: `Derived` and
`Adjudicated` are the repository speaking, `SelfAttested` is an agent vouching
for itself ([[store-binding-record]]). A `depends` written by `sync` under
`Derived` would be the first thing in the system to sit on the wrong side of it.

## When this changes, ask

Does something start deriving one layer from another? Ask which failure mode the
derived copy now has. A memory derived from a fact is redundant; an inference
kept as memory is unreviewed; a memory read out of the inference log is the
checker checking against itself.

Does a verb start applying an inference's processing to a memory — marking a note
`broken` rather than handing it back? That deletes the human judgement the
`--why` seal exists to record, and [[check-drift]] is where the loop is defined.

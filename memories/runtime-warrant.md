---
about:
  - crates/gmr-runtime/src/read.rs#Warrant
  - crates/gmr-runtime/src/read.rs#Holding
  - crates/gmr-runtime/src/read.rs#Knowledge
  - crates/gmr-runtime/src/read.rs#warranted
  - crates/gmr-runtime/src/read.rs#holding
watch: [sig, logic]
---

# What the fact did, and how we know — two axes, because they are both true at once

`Warrant` answers "does this memory's ground still hold". It is a struct of two
enums, not one enum, and the reason is a case that is not rare:

```
a statute was version A when the note was bound
we later observed it change to B          -> the ground moved, established
today the registry is down                -> the last attempt failed
```

Both are true. Report only the outage and the one certain thing here — that it
moved — is thrown away. Report only the move and it claims a currency we do not
have. A flat enum can return one of them.

That case is not a corner. **It is the steady state of every anchor whose probe
has started failing**: the transition is in the log and the observing has
stopped.

```
holding    Holds · Moved{axes, at} · Incomparable{took, reads} ·
           Absent · NeverEstablished · Undated
knowledge  Seen{at, verifiability} · Blind{since, why}
```

`holding` is the main axis, for three reasons. The product answers "do these
grounds still hold", and holding is always relative to the moment of binding.
`bound_at_seq` exists for exactly this comparison and nothing else. And
[[runtime-grounding]]'s main axis is the same shape — `Current` / `Rewritten`
relative to the bound version — with "could we retrieve the old one" as its
annotation, not its sibling.

## This is `Grounding`'s shape, deliberately

`Grounding::Rewritten` carries `before: Before`, and `Before` has its own
`Unreachable`. That design already decided that "the record changed" and "we
could not fetch the old version" are independent, and expressed it by nesting
rather than by making them siblings. `Warrant` is the same decision on the fact
side. Two layers, again: the complete answer, and `bearing()` folding it flat
for counting, exactly as `Grounding::footing()` does.

`bearing()` folds with a precedence, and the precedence has a rule: **blindness
only downgrades a claim that depends on currency.** `Moved` survives it, because
a move we witnessed stays witnessed. `Holds` does not, because "nothing has
moved" is a statement about now, and we cannot see now.

## Where the orthogonal quantities went

Two things that look like they belong in the enum do not, and both are the same
mistake — a scale on a second axis wearing a variant's clothes:

- **Staleness.** `Knowledge::Seen` gives `at` and stops. Whether six hours is too
  old is the caller's threshold, not ours. GMR may take a freshness bound as an
  *observation instruction* — it decides whether to re-probe or serve what it
  has, the way `Budget` decides whether to keep probing — but it never returns a
  verdict about it.
- **Verifiability.** A field on `Seen`, never a variant. It is a grade of how the
  observation was obtained (see [[probe-Verifiability]]), and it is true
  simultaneously with whatever the fact did.

## `Blind` splits three ways because `ReasonClass` does

`Unreachable` / `Unusable` / `Unevaluable` are mapped by an exhaustive match, so
a fourth class cannot be forgotten — the compiler stops it.
[[journal-FailureCode]] is why the split is kept faithful rather than collapsed:
a store that will not answer, a probe whose output cannot be used, and rules that
cannot be evaluated are three different people's problem.

`NeverAsked` is split off from `TimedOut` for the reason [[content-budget]] gives
on the other side: a fact nobody had time to reach is our budget's doing, not
somebody else's outage, and `Footing` already makes the same split for content.

## `Absent` outranks `Moved`, and that costs something

A fact that moved *to* gone is both. `Absent` wins, because "the thing this
memory is about is not there" is the more specific answer and the one a reader
acts on — but the seq and the axes go with `Moved`, so taking `Absent` gives up
saying *when* it vanished. That is a deliberate trade and not a free one; if the
vanishing moment turns out to be what people ask for, the answer is a field on
`Absent`, not a reshuffle of the precedence.

## The diff decides, the seq only gates it

`holding` is **not** `bound_at_seq < moved_at`. That comparison was the first
shape and it was wrong in a way the code could see and did not look at: a
recapture restates the anchor and re-observes it, so it advances `moved_at`
while landing on the state it left. Every dated memory then reported
`Moved` with an **empty** axis list — a claim about the ground contradicted by
the very diff attached to it. One `gmr rebase --all` after an extractor upgrade
says that about the whole corpus at once, which is [[runtime-moved-at]]'s alert
firehose arriving through the other door.

So the state diff decides and `moved_at` only gates it: `bound >= moved_at`
means no state change has happened since the bind, so the fold can be skipped.
Past that gate the answer comes from comparing the two states, and an empty diff
is `Holds` — including when the world moved out and came back, which is the same
early cutoff [[runtime-moved-at]] argues for one level down.

## An instrument that changed is not a world that moved

The diff runs **first**, and an empty one is `Holds` even across two different
extractors. That is not a shortcut: two instruments producing byte-identical
state is positive evidence that they agree about this symbol, and answering
`Incomparable` there would throw it away — this repository would carry hundreds
of unanswerable memories after every extractor upgrade that changed nothing it
extracts.

The question only arises once something differs. Each is read by
whatever extractor was current when it was written, and `Versions::derivation`
records which — so the check is reading the ledger, not interpreting it.
Different versions means a non-empty diff is not commensurable: it would answer
"did the world move" with "the instrument changed shape", and those cannot be
told apart from here.

This is not a hypothetical. This repository's own corpus had 74 memories
reporting `Moved` on axes like `baseline.name` and `v.file` — keys the newer
extractor started emitting — with every `body` hash identical on both sides.
Nothing had moved. `docs/GMR.md`'s blast-radius clause asks exactly this of a
consumer: the three identities are on every entry so that a batch of flips
coming from a rules upgrade can be *identified*, and it says in as many words
that recording the versions without that plan is fixing the record and not the
explosion. `Incomparable` is that plan. It is also the memory-level twin of what
the CLI already says one layer up when a probe version moves — that the stored
baseline and what this build measures cannot be told apart — so it is the same
idea at the layer that needed it, not a new one.

Re-reading it takes a fresh binding, not a recapture: recapture re-pins the
anchor, and the memory is still dated against a reading nobody re-took.

## `Undated` is not `NeverEstablished`

`bound_at_seq` is `NULL` on every binding written before the column existed, and
those rows cannot be compared against the log at all. Answering
`NeverEstablished` said *no ground was ever established* — false about a note
that is bound and whose anchor is settled, and it was the answer for more than
half of this repository's own notes. `NeverEstablished` keeps the case it was
named for: a binding whose seq predates the anchor's first entry, where there
genuinely was no ground yet.

## `axes` is a diff, not a vocabulary

`Moved { axes }` names the **state paths that differ** between the state as of
`bound_at_seq` and the state now. The base computes it without knowing what any
of them mean — `v.sig` is a path, not a word it understands — so this stays
inside rule 4's "no fixed state vocabulary".

`position` and `status` are excluded at the top level. Both are core constants
the base is allowed to know without interpreting (rules 2 and 3): `position` is
where we looked rather than what we found, and `status` is the summary the rule
table already wrote from the rest.

## When this changes, ask

Does a variant arrive that could be true at the same time as another? Then it is
a third axis, and the answer is a field, not a variant. That question is the
whole reason this is a struct.

Does something start deciding `holding` from a seq comparison again? `moved_at`
is a cursor: it says the state changed, never that it changed *away from what
this memory was bound to*. Only the diff says that.

Does `bearing()` grow a precedence that lets blindness erase an established
move? Then the flat view contradicts the structured one, and the flat one is the
one people count.

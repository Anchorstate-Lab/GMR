---
about:
  - crates/gmr-runtime/src/health.rs#Corpus
  - crates/gmr-runtime/src/health.rs#CorpusHealth
  - crates/gmr-runtime/src/health.rs#corpus_health
  - crates/gmr-runtime/src/read.rs#Footing
  - crates/gmr-runtime/src/edges.rs#the_two_corpus_walks_cannot_disagree_about_whether_a_record_is_fine
  - crates/gmr-runtime/tests/operations.rs#a_record_left_behind_by_the_anchor_that_watched_it_is_named
watch: [sig, logic]
---

# "Which anchors are alive" and "which records this corpus holds" are two questions, and one filter used to answer both

`doctor` opened with `let live: Vec<_> = views.iter().filter(|v| !v.closed)` and
then used that slice for everything. For the anchor-level lists — `absent`,
`unseen`, `stranded` — it is the right slice. For the record-level ones it is
not, and the consequence was that `Verdict.gone`, one of the conditions that
turns a run red, was unreachable for any binding whose anchors had all closed.

This repository contained one the whole time. `memories/addr-createSession.md`
had been deleted; `gmr edges` reported `gone: 1` and `gmr doctor` reported
`gone: []`. Two verbs, two answers about the same fact, and the blind one was
the one holding the exit code.

## The fix is not a different slice

Passing `views` instead of `live` is one word, and the next person restores the
filter because "surely we only care about live anchors". So the choice is gone
instead: **record-level facts are only reachable through `CorpusHealth`, which
is computed over every view there is.**

`Corpus` is the value `doctor` now holds. `views()` is every anchor, `live()` is
the open ones — the only filtered thing, and the only place an anchor-level
question is asked — and `health()` is everything about records. There is no
slice for a caller to pick, so picking the wrong one stopped being a judgement
call. `Grounds` and `doctor::grounds` are gone with it.

It also removed a walk. `corpus_health` used to re-read and re-fold every
anchor's journal after `read_all` had already done exactly that, which is two
projections of one log that could disagree with each other.

## `Footing` is the one classifier

`Grounding::footing()` maps the retrieval outcome onto the seven names `doctor`
prints a line for, including the two splits that only `doctor` cared about:
`NeverAsked` (the total content budget ran out first — see [[content-budget]])
and `NoBefore` ([[runtime-grounding]]'s degraded but honest answer).

`edges` still needs the payloads, so `Standing::of` matches the same enum a
second time. Two matches on one shape is exactly what drifted before, so
`the_two_corpus_walks_cannot_disagree_about_whether_a_record_is_fine` walks
every `Grounding` shape and asserts `Standing::of(..).is_some()` iff the footing
is not `Current`. Prose saying "one definition per fact" did not stop the drift;
this does.

## `unsupervised` is the word that was missing

A record is **supervised** iff at least one anchor it names is open. One
predicate, and it catches two situations that look different from the anchor
side and identical from the record side: every anchor closed, and an anchor
that was never opened at all. Walking anchors could only ever see the first,
which is why this is computed from the bindings.

Nothing reported it before. `check` skips `Observed::Closed`, `status` filters
`!closed`, `barren_anchors` counts live anchors only — so the last anchor a note
hangs on closing was how a memory left the supervised set without a word. That
is a note still claiming something about the code with nothing observing it,
which is the state this whole tool exists to make visible.

It is on `Verdict` because it passes [[cli-doctor-run]]'s entry test: the person
holding the repository can supersede the anchor into a new generation (see
[[anchor-Superseded]]), point the note at something still watched, or unbind it.

`memories/constitution.md` is the case that found it. CLAUDE.md was rewritten
from Chinese into English, both `doctrine::` coordinates stopped matching a
heading, `fingerprint` moved them to `absent` — which is terminal — and the two
anchors that watch whether anybody quietly changed the criteria closed. They had
been blind for some time and nothing said so.

## When this changes, ask

Does a new corpus-level count take a slice of views as a parameter? That is the
choice this exists to remove; if it needs one, it is an anchor-level fact and
belongs beside `absent` and `stranded`, not here.

Does a new `Grounding` shape get a `Footing` but no `Standing`, or the reverse?
The agreement test fails on that, and it should — a record one verb calls fine
and another calls broken is how this started.

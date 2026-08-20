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

# "Which anchors are alive" and "which records this corpus holds" are two questions, and only one of them takes a filter

`!v.closed` is the right slice for the anchor-level lists — `absent`, `unseen`,
`stranded`. It is the wrong one for anything about records: a binding does not
stop existing when the anchor it hangs on closes, and a corpus-level count taken
over live anchors silently answers a smaller question than it claims to.

**Record-level facts are reachable only through `CorpusHealth`, which is computed
over every view there is.** That is the guarantee, and it is structural rather
than remembered: `Corpus` hands out `views()` (every anchor), `live()` (the open
ones, for the three questions that want them) and `health()` (everything about
records, already computed). A caller has no slice to pick, so picking the wrong
one is not a judgement call it can get wrong.

`corpus_health` takes the views `read_all` already produced rather than re-folding
every journal, so there is one projection of the log rather than two that can
disagree.

## `Footing` is the one classifier

`Grounding::footing()` maps the retrieval outcome onto the seven names `doctor`
prints a line for, including the two splits that only `doctor` cared about:
`NeverAsked` (the total content budget ran out first — see [[content-budget]])
and `NoBefore` ([[runtime-grounding]]'s degraded but honest answer).

`edges` needs the payloads, so `Standing::of` matches the same enum a second
time. Two matches on one shape is a drift waiting to happen, and prose asking for
"one definition per fact" is not what stops it:
`the_two_corpus_walks_cannot_disagree_about_whether_a_record_is_fine` walks every
`Grounding` shape and asserts `Standing::of(..).is_some()` iff the footing is not
`Current`.

## `unsupervised` is the word that was missing

A record is **supervised** iff at least one anchor it names is open. One
predicate, and it catches two situations that look different from the anchor
side and identical from the record side: every anchor closed, and an anchor
that was never opened at all. Walking anchors could only ever see the first,
which is why this is computed from the bindings.

It is the only thing that speaks when the last anchor a note hangs on closes.
`check` skips `Observed::Closed`, `status` filters `!closed`, `barren_anchors`
counts live anchors only — so without this, a memory leaves the supervised set
without a word, still claiming something about the code with nothing observing
it. That is the state this whole tool exists to make visible, and it is reachable
from the ordinary act of closing an anchor.

It is on `Verdict` because it passes [[cli-doctor-run]]'s entry test: the person
holding the repository can supersede the anchor into a new generation (see
[[anchor-Superseded]]), point the note at something still watched, or unbind it —
and `bind --detach` works even when the record itself is gone, which is the state
that most often produces one (see [[cli-bind-run]]).

## When this changes, ask

Does a new corpus-level count take a slice of views as a parameter? That is the
choice this exists to remove; if it needs one, it is an anchor-level fact and
belongs beside `absent` and `stranded`, not here.

Does a new `Grounding` shape get a `Footing` but no `Standing`, or the reverse?
The agreement test fails on that, and it should: a record one verb calls fine and
another calls broken is worse than either verb being wrong, because the quieter
one is the one CI runs.

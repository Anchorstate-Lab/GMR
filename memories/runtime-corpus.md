---
about:
  - crates/gmr-runtime/src/health.rs#Corpus
  - crates/gmr-runtime/src/health.rs#CorpusHealth
  - crates/gmr-runtime/src/health.rs#corpus_health
  - crates/gmr-runtime/src/read.rs#Footing
  - crates/gmr-runtime/src/read.rs#HoldingKind
  - crates/gmr-runtime/src/read.rs#KnowledgeKind
  - crates/gmr-runtime/src/read.rs#knowledge_of
  - crates/gmr-runtime/src/edges.rs#the_two_corpus_walks_cannot_disagree_about_whether_a_record_is_fine
  - crates/gmr-runtime/tests/operations.rs#a_record_left_behind_by_the_anchor_that_watched_it_is_named
  - crates/gmr-runtime/tests/operations.rs#the_same_record_buckets_under_two_holdings_because_it_hangs_on_two_anchors
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

**Every record-level list names a reference once**, however many assertions
stand behind it. `all()` answers per assertion, so `corpus_health` groups it
through `by_claim` before counting anything — one [[runtime-bound]] per claim,
which makes the property structural rather than a dedup each list has to
remember.

The content lists stay `Ref`-shaped, and drop any claim that is not stored:
`footings` asks whether bytes can still be retrieved, and an utterance has no
bytes, no version, nothing a store could lose.

`unsupervised` is the one list that is claim-shaped, because its question is not
about content at all — it asks whether anything still observes what was claimed,
and an uttered conclusion is exactly as observable as a stored note. For a while
it filtered to `stored()` and a `said:` claim on a closed or never-opened anchor
escaped the census entirely — the only red exit anywhere for that state, gone
because the claim lived in the binding table instead of a store. It does not
grow with every sentence an agent ever says: a conclusion on a live anchor is
delivered, and a retired one has no anchors left to be unsupervised on. What
remains is precisely the set still claiming something that nothing watches.

**The counts read that same delivered set.** `per_anchor`, `barren` and
`unsupervised` all come from `grounded`, never from scanning `all()` for
`binding.anchors.contains(key)`. That scan reads the anchors as *asserted*:
it misses revocations, and it misses a memory carried forward from a
superseded generation, so an heir holding a full corpus reports barren. See
[[store-orset-projection]].

## `Footing` is the content side's classifier

`Grounding::footing()` maps the retrieval outcome onto the eight names `doctor`
prints a line for, including the two splits that only `doctor` cared about:
`NeverAsked` (the total content budget ran out first — see [[content-budget]])
and `NoBefore` ([[runtime-grounding]]'s degraded but honest answer).
`Unverified` is the eighth: a reference no assertion ever cited a version for, so
the bytes came back with nothing to compare them against — see
[[runtime-standing-baseline]].

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

## The fact side has two more, and they are not the same shape

`footings` answers "can these records still be retrieved". For a long time that
was the *only* corpus-level tally, and the fact side — "does the ground these
records were bound to still hold" — had none at all. `doctor` could say twelve
records were gone and could not say a word about how many stood on ground that
had moved. `holdings` and `knowings` close that, keyed by payload-free tags in
the shape `gmr-core` already had for `Change`/`ChangeKind`.

They are filled in the same walk as `footings`, from the `warrant` that
`grounded_all` has already put on every `MemoryView`, so the fact side costs no
extra journal read.

**`holdings` is keyed by anchor and `footings` is not, and that difference is the
point.** A footing is a property of the *record* — the same bytes are retrievable
or not whichever anchor you came from, so one ref, one bucket, and a dedup inside
each bucket is enough. A warrant is a property of the **(record, anchor)
relation**: a note bound to two anchors can be `Holds` on one and `Moved` on the
other, and a test pins exactly that — one note on two anchors, one moved and one
not, asserted to land in both buckets under the anchor that put it there. That
sentence was written here before the test existed and was false for as long as it
stood: `holdings` and `knowings` shipped with no coverage at all, so the shape
this section argues for was resting on the argument alone. Flatten it to `Vec<Ref>` and the same
reference lands in two kinds with nothing saying which anchor put it there —
which is the one question a reader has when they see it.

So `holdings` is `BTreeMap<HoldingKind, BTreeMap<AnchorKey, Vec<Ref>>>`, and its
totals count **pairs, not records**. They do not sum against `footings` and are
not meant to. The shapes differ on purpose: a reader who notices the types are
not parallel has already learned the thing they needed to know.

`knowings` is keyed by neither — it lists `AnchorKey`. `knowledge_of` reads only
`faltering`, `last_sighting` and `derivation`, all of them anchor-level, so the
whole axis is the same for every memory on an anchor. Listing refs would repeat
one anchor's outage once per note bound to it.

`knowledge_of` is a function rather than a body inside `warranted` for the reason
this file keeps returning to: `doctor` used to derive blindness a second way, as
`faltering.is_some()`, and two derivations of one fact is the drift this corpus
walk exists to avoid. It happens to be exactly equivalent — after `Open` every
anchor has a `last_sighting` and a `derivation`, so `faltering: None` is always
`Seen` — but equivalent-by-accident is not a property anything checks. Now
`doctor`'s `unseen` is the union of the four blind kinds, and the split into
`Unreachable` / `Unusable` / `Unevaluable` is finally printed, which is what
[[render-warrant]] asks for: three different people's problem, said three ways.

Neither is on `Verdict`. They fail [[cli-doctor-run]]'s entry test the way
`Footing::Unreachable` does — ground moving is `check`'s sentence and `check`
already exits on it, and two commands going red for one fact is the drifting
second copy in exit-code form. Adding a kind to `Verdict` later is additive;
taking one out breaks somebody's CI.

## When this changes, ask

Does a new corpus-level count take a slice of views as a parameter? That is the
choice this exists to remove; if it needs one, it is an anchor-level fact and
belongs beside `absent` and `stranded`, not here.

Does a new `Grounding` shape get a `Footing` but no `Standing`, or the reverse?
The agreement test fails on that, and it should: a record one verb calls fine and
another calls broken is worse than either verb being wrong, because the quieter
one is the one CI runs.

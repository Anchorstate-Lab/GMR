---
about:
  - crates/gmr-runtime/src/read.rs#ground
  - crates/gmr-runtime/src/read.rs#Standing
  - crates/gmr-runtime/src/read.rs#Anchored
  - crates/gmr-runtime/src/read.rs#Evidence
  - crates/gmr-runtime/src/read.rs#stood_all
  - crates/gmr-runtime/src/read.rs#records_of
  - crates/gmr-runtime/src/read.rs#anchored
  - crates/gmr-runtime/src/read.rs#Shown
  - crates/gmr-runtime/src/read.rs#shown_at
  - crates/gmr-runtime/src/read.rs#recorded_at
  - crates/gmr-runtime/src/read.rs#Reading
  - crates/gmr-runtime/src/read.rs#sample
  - crates/gmr-runtime/tests/grounding.rs#a_sentence_bound_to_the_reading_it_was_shown_says_which_one
  - crates/gmr-runtime/tests/grounding.rs#a_sentence_citing_a_reading_this_anchor_never_took_is_not_grounded_by_it
  - crates/gmr-runtime/tests/grounding.rs#a_sentence_that_cited_no_reading_is_not_reported_as_having_missed_one
  - crates/gmr-runtime/tests/grounding.rs#one_sentence_on_four_anchors_comes_back_with_four_warrants
  - crates/gmr-runtime/tests/grounding.rs#both_phases_of_one_call_run_against_one_deadline
  - crates/gmr-runtime/tests/operations.rs#grounding_reads_the_whole_log_only_when_the_binding_predates_the_move
watch: [sig, logic]
---

# Two answers with different cardinalities, so they cannot live in one struct

`ground(asked, how)` is keyed by claim because that is what a caller has: a
sentence it is about to say, or one it just said. Everything else here is keyed
by anchor, and the difference is not cosmetic — the two things a caller asked
for are counted differently:

```
Grounding   is the text still there, still the same     one per claim     IO
Warrant     what is this anchor's observation state     one per (claim, anchor)   pure
```

`Standing.record` is `Option<Grounding>` and the `None` is not a failure: a
`Claim::Said` is stored nowhere ([[memory-Binding]]), so there is no document to
fetch and no version to compare. Reporting a grounding there would be answering
about a file nobody wrote.

`Standing.reached` is the fourth axis and the only optional one: what this claim
rests on **through its own citations**, and which of those have moved. It is
empty unless the caller asked, for the reason [[runtime-reaching]] gives, and it
carries no verdict — a record that moved is reported with the path that led to
it, and what that means for the claim is the reader's.

`Standing.depends` is the third: the invariant the asserter wrote down, over all
the anchors at once. `Holding` is per anchor and `depends` is per claim, which is
why one is inside `on` and the other beside it.

`MemoryView.warrant` is one `Option<Warrant>`, and it is *correct* for the
question `grounded(key)` asks — how does this record stand **with respect to
this anchor**. It cannot answer the reference-keyed question at all, because
there is no single anchor to be relative to. Most notes in this repository
bind more than one.

So `Standing` is a separate type rather than a reshaped `MemoryView`, and
[[runtime-read]] is why: `MemoryView` carries `grounded`, `baseline_at` and
`links` for the CLI to render, and it will keep growing that way. A contract
type behind an earned-hash guard is the wrong place for fields that exist so
a terminal can print something. The cost of two types is one shape written
twice; the cost of one is that the next rendering field arrives inside a
versioned promise.

## The two phases do not depend on each other, so they do not wait for each other

Phase 0 turns references into bindings and a deduplicated anchor set. After
that, observing those anchors and fetching those records are two chains that
touch different stores and share no input. They run under one parent budget
minted once, each phase `narrowed_to` its own span and its own output cap
(see [[probe-budget]]).

Run in sequence, "give me 200ms" means 200 for looking at the world and 200
more for reading the sentence, and answering the first question well is a
way to run out of time for the second. There is no good rule for splitting
one deadline between two phases that do not need each other, and the fix is
not to find one.

The test does not time anything. It hands both a span shorter than either
phase's own limit, which clamps both to the parent's instant — an equality
that only holds if they were minted together.

## A reference nobody can justify is an answer, not a fault

A batch where one bad reference raises `Err` loses the nineteen good answers
and the order that lets a caller match them to what it asked. Only our own
failures — the store, the canonicaliser — end the call. Everything about a
particular reference comes back in that reference's own slot:

```
on is empty                  nothing anchors this; go bind it, or it is not ours
Anchored::Unopened { key }   bound to a key nothing ever opened; go fix the binding
Anchored::On { .. }          the warrant, and what to go and check it with
```

`Unopened` is a variant rather than a `Warrant` shaped to say it, because
`Knowledge::Blind { why: NeverAsked }` would be a lie in the direction that
costs the most: it tells a caller to wait for an observation that is never
coming. Adding a `Blind` variant instead would have deformed a contract type
to describe our own bookkeeping.

## Where an ask gets its anchors, its readings and its invariant

An `Asked` names a claim, and may name the anchors it rests on, the readings it
cited and the invariant it asserts. A claim with an assertion in the store takes
those three from the store; one with no assertion takes them from the ask, and
the ask is not written anywhere. A sentence about to be said has nothing in the
store yet, and it is exactly as answerable as one that does.

That an inference is checkable without first becoming a record is
[[three-layers]]: a memory is a long-lived constraint, written down and
reviewed; an inference is one turn's conclusion and needs nobody. Storing every
answered question would make each one an assertion no one reviewed.

The two sources are mutually exclusive and **the data decides which**, not the
caller: an asserted claim asked about inline is `AlreadyAsserted`. If the ask
won, the answer would be about something nobody recorded; if the store won, what
the caller passed would be silently ignored. Both leave two callers holding
different answers about one claim with nothing reporting the disagreement.

## `Evidence` names what to go and check, never the value

`reading`, `ProbeVersion`, `bound_at`, `moved_at`, `saw`, `shown`. Enough to
recompute, to compare, to walk back to the entry — and structurally incapable of
being used as a data source, because the reading itself is not in it. GMR
neither caches business data nor promises its freshness; handing it back would
make it a bad database, and a field named `evidence` full of values would get
used as one whatever it was called.

`Anchored::On` boxes its warrant and its evidence. `Unopened` carries a key and
nothing else, and the gap was wide enough that a `Vec<Anchored>` was mostly
padding.

## `shown` asks whether the answer and the anchor looked at the same thing

`saw` is the fact address the asserter cited ([[store-binding-record]]);
`anchored` matches it against the anchor's own `Open` and `Transition` entries
and reports one of three things:

```
Seen { at }        this anchor recorded that exact reading, and was still
                   showing it when the claim was bound
Superseded { at }  a real reading, already replaced when the claim landed
Unseen             it cited a reading this anchor never took
NotSaid            it cited none, which is what a note a person wrote does
```

`Superseded` is the same defect as `Unseen` arriving late: the reading is real,
but the anchor had moved past it before the conclusion was recorded, so the
conclusion was built on the anchor's past. The comparison point is the binding's
own `bound_at_seq` — fold the journal to that seq and ask whether any cited
address is the reading the anchor was showing *then*. Later movement stays
`Moved`'s answer, and a test pins the honest path: a citation current when bound
stays `Seen` however far the world moves afterwards. Content addressing gives
this one free kindness — a world that flaps back restores the old address, and a
citation of it is `Seen` again, because the anchor really is showing that
reading. An undated binding has no comparison point and stays `Seen`; honest,
because nothing can be said either way.

`Unseen` is the shape of a **second computation of the same fact**, running
beside the anchor instead of through it. That is not hypothetical: a probe
rewriting a product's pricing rules in SQL agreed with the product until it did
not, within hours, and every answer built on it still came back holding. The
`sample` verb exists so the delivery path and the anchor are one look at the
world rather than two — read the anchor, build the answer from what it returned,
cite the address it came with.

**`Holding` is deliberately untouched by this.** It answers whether what the
anchor established has moved; `shown` answers whether this claim was built from
what the anchor established. Folding the second into the first would leave a
reader unable to tell a fact that changed from an answer assembled somewhere
else, and those want opposite responses — re-ask the world, versus fix the
delivery path. A test asserts `Unseen` and `Holds` together for exactly that
reason.

## A lease held elsewhere is not this call's problem

Phase 1 swallows `RuntimeError::Leased` and reports what stands. Since
[[store-queue-fence]] the lease is a device for not firing the same probe
twice, so somebody else holding it means the observation is being made —
just not by us. Failing the call would be a request going red because a
scheduler was running, and the honest answer is already in hand:
`Knowledge::Seen { at }` says exactly how old the reading is.

## Phase one walks an owned list

`stream::iter` is handed `keys.to_vec()`, not `&keys`. A borrowed slice iterator
inside a `buffered` stream lives across every await in the batch, and that is
enough to make the whole `ground` future not `Send` — which no test noticed until
a host tried to spawn it. The clone is a handful of short keys, once per call,
and it has a semantics: the stream owns the list it walks. See [[hosts-spawn]].

## When this changes, ask

Does something start deciding, ranking, or folding these two axes into one
number? That is the caller's reduction and the structure-not-entailment boundary
(`docs/ARCHITECTURE.md` §1.4, [[gmr-not-entailment]]) puts it outside — [[runtime-warrant]] records what happened the last time one was
shipped from here.

Does a phase start depending on the other's output? Then the parallel is
gone and the budget question comes back, and it still has no good answer.

Does a caller start reading the world itself and citing a `saw` it computed
rather than one `sample` handed back? Then `Unseen` is what it will get, which
is the correct answer and not a bug to route around.

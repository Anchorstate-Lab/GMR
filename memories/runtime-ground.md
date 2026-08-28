---
about:
  - crates/gmr-runtime/src/read.rs#ground
  - crates/gmr-runtime/src/read.rs#Standing
  - crates/gmr-runtime/src/read.rs#Anchored
  - crates/gmr-runtime/src/read.rs#Evidence
  - crates/gmr-runtime/src/read.rs#stood_all
  - crates/gmr-runtime/src/read.rs#records_of
  - crates/gmr-runtime/src/read.rs#anchored
  - crates/gmr-runtime/tests/grounding.rs#one_sentence_on_four_anchors_comes_back_with_four_warrants
  - crates/gmr-runtime/tests/grounding.rs#both_phases_of_one_call_run_against_one_deadline
  - crates/gmr-runtime/tests/operations.rs#grounding_reads_the_whole_log_only_when_the_binding_predates_the_move
watch: [sig, logic]
---

# Two answers with different cardinalities, so they cannot live in one struct

`ground(refs, how)` is keyed by reference because that is what a caller has:
a sentence it is about to say. Everything else here is keyed by anchor, and
the difference is not cosmetic — the two things a caller asked for are
counted differently:

```
Grounding   is the text still there, still the same     one per reference     IO
Warrant     what is this anchor's observation state     one per (reference, anchor)   pure
```

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

## `Evidence` names what to go and check, never the value

`fact_address`, `ProbeVersion`, `bound_at`, `moved_at`. Enough to recompute,
to compare, to walk back to the entry — and structurally incapable of being
used as a data source, because the reading itself is not in it. GMR neither
caches business data nor promises its freshness; handing it back would make
it a bad database, and a field named `evidence` full of values would get
used as one whatever it was called.

## A lease held elsewhere is not this call's problem

Phase 1 swallows `RuntimeError::Leased` and reports what stands. Since
[[store-queue-fence]] the lease is a device for not firing the same probe
twice, so somebody else holding it means the observation is being made —
just not by us. Failing the call would be a request going red because a
scheduler was running, and the honest answer is already in hand:
`Knowledge::Seen { at }` says exactly how old the reading is.

## When this changes, ask

Does something start deciding, ranking, or folding these two axes into one
number? That is the caller's reduction and `docs/GMR.md`'s boundary puts it
outside — [[runtime-warrant]] records what happened the last time one was
shipped from here.

Does a phase start depending on the other's output? Then the parallel is
gone and the budget question comes back, and it still has no good answer.

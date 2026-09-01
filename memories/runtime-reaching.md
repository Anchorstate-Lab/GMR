---
about:
  - crates/gmr-runtime/src/link.rs#Reached
  - crates/gmr-runtime/src/link.rs#reaching
  - crates/gmr-runtime/src/link.rs#REACHED_AT_MOST
  - crates/gmr-runtime/tests/grounding.rs#what_a_memory_rests_on_is_followed_and_a_broken_link_comes_back_with_its_path
  - crates/gmr-runtime/tests/grounding.rs#following_links_is_asked_for_and_bounded_by_the_caller
  - crates/gmr-runtime/tests/grounding.rs#a_cycle_in_the_links_is_walked_once_and_not_forever
watch: [sig, logic]
---

# A memory rests on the memories it cites, and until now nothing followed that

Links between records were recorded, listed, and rendered, and they never
reached grounding. A note whose whole argument was "because [[layers]] says so"
kept standing after `layers` was rewritten underneath it, and the only thing
that would ever have caught it was a person remembering the citation.

`reaching` walks link edges from a claim's stored record and reports the ones
whose footing is **not** `Current`, with the path that led there. That is the
skeleton of a truth-maintenance system's retraction pass, and deliberately only
the skeleton: it says *this claim reaches a record that moved, by this route*.
It does not say the claim is therefore wrong. See [[gmr-not-entailment]] — the
structure is ours, the entailment is not.

## Only what moved, and the path that led to it

Reporting every record touched would bury the one that matters, and a corpus
where nothing moved reports nothing at all. `via` is the kinds traversed in
order, because "something you cite has changed" is not actionable and "your
second citation, through `contradicts`, has changed" is.

`LinkKind` is not interpreted, which is rule 4 one layer out: this walk follows
every kind and hands the names back. A domain that means something by `cites`
and something else by `contradicts` reads them off `via`; the base that decided
for it would be inventing a vocabulary the anchor layer refuses to have.

## Three bounds, and each one is load-bearing

**Asked for.** `Instructions.reach` is absent by default. The walk is a store
read per record, on a path that runs on every `ground`, and a cost nobody
budgeted is a cost that shows up as an unexplained slowdown in somebody else's
product.

**Depth.** The caller says how far, and the walk stops there. It is a bound, not
a hint: at depth one the test asserts a record two hops out is *not* reported.

**Visited once, and at most `REACHED_AT_MOST`.** Two memories citing each other
is the ordinary shape of a corpus, not a defect — without the seen-set this call
does not return. The node cap is the other half: a depth bound alone does not
bound fan-out, and a hub record everything cites would fan a shallow walk into
thousands of store reads.

The budget is the caller's total, narrowed per record, for [[content-budget]]'s
reason — this half of the walk is the half nobody explicitly asked about.

## An utterance reaches nothing

Links run between records that live somewhere. A `Claim::Said` is stored
nowhere ([[memory-Binding]]), so nothing links to or from it, and `reached` is
empty however far the caller asks. What an utterance rests on is its anchors and
its `depends`, not a citation graph.

## When this changes, ask

Does the walk start reporting `Current` records too? Then the answer grows with
the corpus rather than with what is wrong in it, and the field stops being read.

Does a `LinkKind` start meaning something here — followed, skipped, weighted?
That is a state vocabulary in the base, and the domain that disagrees with it
has no way to say so.

Does `reached` grow a verdict — "and therefore this claim is stale"? That is
entailment, and the one thing this system says it does not do.

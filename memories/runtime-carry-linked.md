---
about:
  - crates/gmr-runtime/src/memory.rs#carry_linked
  - "crates/gmr-runtime/src/read.rs#ground"
watch: [sig, logic]
---

# Carrying linked records is asked for, one hop, and marked

`carry_linked` pulls in memories linked from an already-fetched one and not
themselves delivered under this anchor — links run between stored records, so
it asks the binding table for `Claim::Stored` and nothing else — and marks
every one `grounded: false`. Being bound to some other anchor somewhere is not
being about this one, so the far end of a link gets no guarantee here however
solidly it stands elsewhere; hiding that would let a link launder a foreign
binding into a local one. A carried record also gets no `warrant`: there is no
binding seq under this anchor, so "moved since bound" has nothing to mean
([[runtime-read]]).

The walk runs only when the caller asks — `Instructions.carry`, gated in
`ground` — for the same reason `reach` is opt-in on the claim path: a store
read per record, on the path every read takes, is not something a caller
should pay for without having asked, and a delivery that mixes what merely
mentions a bound note into what is about the anchor dilutes exactly the
attention it exists to focus. The caller says whether to walk, and silence
says not at all.

The walk stops at one hop on purpose, and that has not changed. Deciding "is
that distant memory still meaningfully about this anchor" is a judgment about
**relevance**, and the substrate has no basis to make it; it belongs to the
domain.

`ground`'s `reach` does walk further, and it is not this question wearing a
different name. Delivery asks *what should I hand this reader* — an answer that
gets longer and vaguer with every hop. Propagation asks *has anything this rests
on itself moved* — an answer that gets **shorter**, because only what is not
`current` is reported and a corpus where nothing moved reports nothing. The
first is relevance and stays here; the second is structure and lives in
[[runtime-reaching]].

Records carried in this way draw from the same operation-wide budget as the
bound ones — `carry_linked` takes the total and narrows a slice per record
rather than minting its own (see [[content-budget]]). A second total here
would mean a read could spend twice what its caller allowed, and the
carried records are the half of the walk nobody explicitly asked for
record by record.

## When this changes, ask

Does the new code let `carry_linked` recurse into a second hop, ever mark a
linked-in record `grounded: true`, or run the walk without `carry` being
asked for? Each hands the substrate a judgment it is not positioned to make
correctly — relevance, aboutness, and cost belong to the caller and the
domain. If what is wanted is "and what has moved further out", that is
`reach`, and it answers a different question.

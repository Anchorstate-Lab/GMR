---
about: crates/gmr-runtime/src/memory.rs#carry_linked
watch: [sig, logic]
---

# Grounding does not propagate along links, and a link is only followed one hop

`carry_linked` pulls in memories linked from an already-fetched one and not
themselves delivered under this anchor — links run between stored records, so
it asks the binding table for `Claim::Stored` and nothing else — and marks every one `grounded:
false`. The test for pulling one in is that it has any assertion at all —
"known to GMR", not "currently on an anchor" — so a record whose every tag
has been revoked is still carried and still marked ungrounded, which is
exactly the state a reader has to be able to see. That flag has to stay visible on the carried-in
record: a link is not a promise that the far end is still about the anchor
the near end is about, so a carried memory gets no guarantee, and hiding
that would let an unanchored record masquerade as a grounded one just
because something linked to it.

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
carried records are the half of the walk nobody explicitly asked for.

## When this changes, ask

Does the new code let `carry_linked` recurse into a second hop, or does it
ever mark a linked-in record `grounded: true`? Either one hands the
substrate a judgment it is not positioned to make correctly. If what is
wanted is "and what has moved further out", that is `reach`, and it answers a
different question.

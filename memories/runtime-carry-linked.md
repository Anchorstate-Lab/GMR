---
about: crates/gmr-runtime/src/memory.rs#carry_linked
watch: [sig, logic]
---

# Grounding does not propagate along links, and a link is only followed one hop

A linked reference is carried when it has any assertion at all, and dropped
when it has none — "known to GMR" is the test, not "currently on an anchor".
A record whose every tag has been revoked is still carried, and still marked
ungrounded, because that is exactly the state a reader has to be able to
see.

`carry_linked` pulls in memories that are linked from an already-fetched
memory but are not themselves bound to any anchor — and marks every one of
them `grounded: false`. That flag has to stay visible on the carried-in
record: a link is not a promise that the far end is still about the anchor
the near end is about, so a carried memory gets no guarantee, and hiding
that would let an unanchored record masquerade as a grounded one just
because something linked to it.

The walk stops at one hop on purpose. Following further would require
cycle handling, and — more importantly — deciding "is that distant memory
still meaningfully about this anchor" is a judgment call about relevance
that the substrate has no basis to make; it belongs to the domain, not to
this function.

Records carried in this way draw from the same operation-wide budget as the
bound ones — `carry_linked` takes the total and narrows a slice per record
rather than minting its own (see [[content-budget]]). A second total here
would mean a read could spend twice what its caller allowed, and the
carried records are the half of the walk nobody explicitly asked for.

## When this changes, ask

Does the new code let `carry_linked` recurse into a second hop, or does it
ever mark a linked-in record `grounded: true`? Either one hands the
substrate a judgment it is not positioned to make correctly.

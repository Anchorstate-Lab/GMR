---
about:
  - crates/gmr-content/src/lib.rs#MemorySource
  - crates/gmr-content/src/lib.rs#Record
  - crates/gmr-content/src/lib.rs#Claim
watch: [sig, logic]
---

# A contract the base defines and never calls

Nothing in `gmr-core`, `gmr-runtime` or any other base crate calls
`MemorySource`. It is here so a battery and a domain can agree on the shape
of "what records are in this store" without either of them owning the
vocabulary — the same reason `ContentProvider` is here rather than in the
battery that implements it.

**Which store to enumerate, and how much of it, is a domain decision.** A
markdown source scans one directory; a mem0 source has a `user_id` and an
`agent_id` and possibly a page size. None of that is the base's business,
so instances are constructed and held on the domain side, not registered on
`Runtime`. An earlier draft of this work put them on the builder so that
`edges` could take one `list()` snapshot instead of N fetches; that was
dropped because it created a second path deriving current versions, which
is exactly what [[runtime-current-version]] exists to prevent.

## `list()` is discovery help, not a roster of what exists

A reference missing from `list()` is **not** `Gone`. mem0 filters by scope,
a markdown source scans one directory, and any store may paginate — an
absence here means "not in this listing", nothing more. Only `fetch`
answering `Ok(None)` is authoritative about a record being gone, and
[[runtime-grounding]] is where that answer turns into something a reader
acts on. Treating a short listing as a set of dead references would produce
a screenful of records to delete that are all still there.

## `Claim` is a capability of the source, not an obligation of the record

Some stores let a record say what it is about: markdown frontmatter is
exactly that. Most do not, and requiring it would push adoption onto
whether someone can change the agent that writes their memories — memories
are often written by a different agent entirely, and mem0's own update path
makes no promise about metadata surviving. So `Silent` is a first-class
answer and the ordinary one; a source that has no notion of a claim returns
it for every record and nothing is wrong.

Declarations for such stores go through `gmr bind`, which is the base
primitive anyway: the binding table has always been the authority on which
anchors a record is about, and it lives beside the store rather than in it.

`Malformed` exists so the one case that *can* be wrong stays wrong loudly.
A source that finds a claim it cannot parse must not silently downgrade to
`Silent` — that would turn a typo in frontmatter into "this note declares
nothing", which is a lint the domain already publishes under `unclaimed`
and would then be reporting for the wrong reason.

## When this changes, ask

Does a base crate start calling `list()`? Then discovery has become a base
mechanism, and the next question is which store it enumerates — which the
base cannot answer without making a domain's decision for it.

Does `Silent` acquire a meaning beyond "this record says nothing"? It is
returned by every record in stores that have no claims at all, so any
consequence attached to it lands on all of them at once.

---
about:
  - crates/gmr-content/src/lib.rs#MemorySource
  - crates/gmr-content/src/lib.rs#Declaring
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

`list()` takes a budget like every other outbound call. Enumerating a
remote store is the least bounded thing in this contract — it is the one
call whose cost does not depend on how many bindings exist — so it is the
last place to leave unbounded (see [[content-budget]]).

## `list()` is discovery help, not a roster of what exists

A reference missing from `list()` is **not** `Gone`. mem0 filters by scope,
a markdown source scans one directory, and any store may paginate — an
absence here means "not in this listing", nothing more. Only `fetch`
answering `Ok(None)` is authoritative about a record being gone, and
[[runtime-grounding]] is where that answer turns into something a reader
acts on. Treating a short listing as a set of dead references would produce
a screenful of records to delete that are all still there.

## Declaring is a second trait, because it was a returned "I have none"

Some stores let a record say what it is about: markdown frontmatter is
exactly that. Most do not, and requiring it would push adoption onto whether
someone can change the agent that writes their memories — memories are often
written by a different agent entirely, and mem0's own update path makes no
promise about metadata surviving. Declarations for such stores go through
`gmr bind`, which is the base primitive anyway: the binding table has always
been the authority on which anchors a record is about.

`Claim` used to be a field on `Record`, and a store with no notion of a
declaration returned `Silent` for every one of its records. That is the
shape D3 removed from `History` — **an absent capability expressed as a
value every caller has to handle** — and it grew back here unnoticed.

What it cost was measured rather than argued. Pointed at a store that
declares nothing, the domain read 147 records, produced zero anchors, and
`sync` and `check` both exited 0: a repository supervising nothing, with
every gate green. The callers were all handling `Silent` correctly. There
was simply nothing in "this record says nothing" to distinguish from "this
store has no way to say anything", and the second is a misconfiguration
while the first is ordinary.

So `Claim` moved onto `Declaring::claim_of`, and `Record` no longer carries
one. A store that does not declare does not implement the trait, and the
declaration path cannot be handed it at all. `Silent` keeps its original and
now unambiguous meaning: within a store that does declare, this particular
record declares nothing.

## `Declaring` is synchronous, and that is its whole admission test

Its methods take no `Budget` and return no future, so a store reachable
only across a network cannot implement it. That is deliberate and it is the
one place where the control plane and the data plane are separated by the
compiler rather than by a rule someone has to remember.

The reason is a failure with no good handling. `Grounding::Unreachable`
covers a store that will not answer for a record's *content*: D6 puts it in
the bucket that never turns red, `read` reports it, and the anchor is still
judged. There is no equivalent for a store that will not answer about which
anchors *exist* — without that answer there is no roster to judge, so
`check` and `doctor` can only fail outright. Measured, with the declarations
in a remote store and the store stopped: `read` exited 0 and reported each
binding as unreachable; `check` and `doctor` exited 2.

Exit 2 was the correct code. What was wrong was that the declarations could
be put somewhere unreachable at all.

`Malformed` exists so the one case that *can* be wrong stays wrong loudly.
A source that finds a claim it cannot parse must not silently downgrade to
`Silent` — that would turn a typo in frontmatter into "this note declares
nothing", which is a lint the domain already publishes under `unclaimed`
and would then be reporting for the wrong reason.

## When this changes, ask

Does a base crate start calling `list()`? Then discovery has become a base
mechanism, and the next question is which store it enumerates — which the
base cannot answer without making a domain's decision for it.

Does `Silent` acquire a meaning beyond "this record says nothing"? It once
also meant "this store cannot say anything", and the two are a
misconfiguration and an ordinary day.

Does `Declaring` gain an `async` method, or a `Budget`? That is the moment
the anchor roster becomes something a network can withhold, and no exit code
can express that usefully — see [[cli-notes-source]] for why the one
implementation was already synchronous before the contract required it.

---
about:
  - crates/gmr-content/src/lib.rs#MemorySource
  - crates/gmr-content/src/lib.rs#MemoryStore
  - crates/gmr-content/src/lib.rs#Record
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

## `MemoryStore` is what a battery hands back

The one contract, plus whichever capabilities that backend has. It exists
because "can this store be enumerated" was knowledge held by whoever wired
the store up, and that knowledge was three hand-written branches in one
`main.rs` — each a different shape, so the fourth backend was going to be a
copy of the third.

Configuration deliberately is *not* uniform: a git store needs a repository
root, a mem0 store needs a key and a scope. That asymmetry is the domain
deciding which store to talk to, which is the decision this crate declines
to make. What is uniform is the return type.

## Declaring left, and what would bring it back

There was a `Declaring` trait here, for stores whose records can say what
they are about — markdown frontmatter is exactly that. It had one
implementation, in the domain, and after `name_of` moved out, not one call
site dispatched through it: every caller held the concrete type. A trait in
the base that no base crate calls, no battery implements, and nobody
dispatches on, is the base carrying a domain's vocabulary.

`MemorySource` earns the same address that `Declaring` did not: two
implementations across two layers, and `gmr memories` dispatches on it. The
contrast is the test, and it is worth applying to the next capability
someone wants to add here.

`Claim` and `Stated` are domain types now (see [[cli-notes-source]]). What
they encode did not change and neither did why:

- `Claim` was once a field on `Record`, and a store with no notion of a
  declaration returned `Silent` for every record it had. That is the shape
  D3 removed from `History` — **an absent capability expressed as a value
  every caller has to handle**. Measured before it was fixed: pointed at a
  store that declares nothing, the domain read 147 records, produced zero
  anchors, and `sync` and `check` both exited 0. Every caller was handling
  `Silent` correctly; there was simply nothing in "this record says nothing"
  to tell apart from "this store cannot say anything", and the second is a
  misconfiguration while the first is an ordinary day.
- The record and its claim arrive **together**. Split into `records()` then
  `claim_of(&record)`, the second call sees only bytes, so a file that would
  not open and an empty file both arrive as none — and the diagnosis has to
  be recovered by reading the store a second time.
- It is **synchronous and takes no `Budget`**, so a store reachable only
  across a network cannot declare. `Grounding::Unreachable` covers a store
  that will not answer for a record's *content*: D6 keeps it out of every
  exit code, `read` reports it, the anchor is still judged. There is no
  equivalent for a store that will not answer about which anchors *exist* —
  without that answer there is no roster to judge at all. Measured, with the
  declarations in a remote store and the store stopped: `read` exited 0 and
  reported each binding unreachable; `check` and `doctor` exited 2. Exit 2
  was the right code; what was wrong was that declarations could be put
  somewhere unreachable at all.

**What brings the trait back here: a second store that can declare, in a
battery.** A git-backed local memory directory (Letta Code's MemFS is one)
is the shape to expect. When it arrives the trait belongs in this crate
again, and the three properties above are what it has to keep — the third
one especially, because it is the only one a compiler can hold.

## When this changes, ask

Does a base crate start calling `list()`? Then discovery has become a base
mechanism, and the next question is which store it enumerates — which the
base cannot answer without making a domain's decision for it.

Does `Silent` acquire a meaning beyond "this record says nothing"? It once
also meant "this store cannot say anything", and the two are a
misconfiguration and an ordinary day.

Does a capability arrive here with one implementation and no dispatch? That
is what `Declaring` was, and the answer is that it lives where its one
implementation lives until a second one turns up somewhere this crate can
see.

Does `MemoryStore` grow a field that only one backend can fill? It carries
capabilities, and a capability nobody else can have is that backend's own
business, not a slot everyone else leaves empty.

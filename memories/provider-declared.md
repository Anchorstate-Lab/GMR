---
about:
  - batteries/provider/src/declared.rs#Declared
  - batteries/provider/src/declared.rs#Listing
  - batteries/provider/src/declared.rs#body
  - batteries/provider/src/declared.rs#refused
watch: [sig, logic]
---

# A store can be taught to GMR without teaching it to the compiler

Every other provider in this battery is a backend somebody wrote Rust for.
This one is a backend somebody wrote a *script* for: the recipe names an
executable, GMR runs it, and what comes back on stdout is the memory.

The seam is `gmr_probe::Transport`, unchanged and not extended. A probe call
and a content fetch are the same shape — a position in, `Found { facts }` or
`NotFound` out — and the split that matters most here was already drawn on
that trait: `NotFound` is the world's answer, `Err` is our failure. A
declared provider inherits [[runtime-grounding]]'s three-way split for free
rather than reconstructing it.

The transport is injected, never constructed here, so this battery depends
on the contract and not on any transport that implements it. Which
executable answers for a provider is the domain's business
([[cli-providers-recipe]]).

## Three things this shape cannot do, and each is said rather than worked around

**A memory is one JSON string.** `Facts` is JSON; a memory is bytes. `body`
takes `text` and nothing else, so a store holding records that are not text
cannot be declared this way and is told so at the first read rather than
handed back mojibake. Carrying bytes would mean base64 — a dependency and a
second spelling of every record — and no store this was built for holds
anything but text.

**The version is ours, never the store's.** `Transport::invoke` has no
channel for one: `Outcome` carries facts, and there is nowhere to put a
native revision id. So `body` computes a content hash over the bytes. That
satisfies both halves of the law every provider owes
([[content-conformance]]) and costs the store's own version, which is why
**git cannot move here**: its version is a git blob hash, and a version this
crate computed instead would differ from the one the provider hands back,
reporting every binding as rewritten forever.

**A fetch is a process.** One fork per record read. That is affordable
because [[content-budget]] already bounds both a single call and the whole
run, and a declared store is subject to those bounds like any other.

## `Listing` is a separate type because declining is not answering

A recipe with no listing script produces a `MemoryStore` with no
`MemorySource` at all, which is what [[content-discovery]] settled: a store
that cannot enumerate declines the trait rather than returning an empty
list. So `Listing` exists only when a listing script does, and nothing here
can be asked a question it has no way to answer.

A listing script that answers `null` is refused rather than read as an empty
store: `null` is this contract's spelling of "no such record", and a store
holding nothing lists `{"records": []}`.

## `refused` keeps the one distinction the budget depends on

`ProbeErrorCode::TimedOut` becomes `ContentError::spent`, everything else
`ContentError::new`. That is not tidiness: `BudgetSpent` is what
[[content-budget]] turns into `Footing::NeverAsked`, a partial view the
reader is told to widen, while any other failure is a store that would not
answer. Collapsing them tells a reader their budget is fine when it is not.

## When this changes, ask

Does a `bytes` or `base64` field get added to the contract? Then every
record has two spellings and a version derived from whichever one arrived —
decide which is canonical before, not after.

Does anything start deriving the version from a field the script returns?
That reintroduces exactly the failure that keeps git compiled in.

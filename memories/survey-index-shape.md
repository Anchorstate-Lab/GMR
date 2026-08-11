---
about:
  - batteries/survey/src/index.rs#Generation
  - batteries/survey/src/index.rs#under
  - batteries/survey/src/index.rs#Indexed
  - batteries/survey/src/index.rs#Index
  - batteries/survey/src/testkit.rs#Remembered
watch: [sig, logic]
---

# One index per probe and version; the root is a predicate, and the writer owns the order

Nothing calls any of this yet. It lands ahead of the extractors that will use
it, for the same reason [[survey-narrow]] did: two backends that have to agree
should be able to disagree in a test before either has a caller.

## The root is not part of the address

`Generation::of(probe, version)` — and nothing else. In particular **not the
root**, which is where the cache it replaces went wrong.

The cache keys scopes `probe@stamp@root`, and this repository opens seven
`layer::*` anchors whose `params.root` narrows `ast-map` to one crate each (see
[[layers]]). So a file under `crates/gmr-runtime` is stored once for the root
scope and again for its layer's. Measured on this repository: 4.5 MB of distinct
content held as 6.5 MB, and the redundancy grows with every layer anchor
somebody opens. Opening one should cost zero index bytes, because it asks a
different question about the same facts — it does not create new ones.

So `root` moves to the query. `under(rel, root)` decides it, and the thing it
has to get right is that a root selects what is *beneath* it, not what shares
its opening characters: `crates/gmr-core` must not draw in
`crates/gmr-core-extra`. A `LIKE 'root%'` in any backend gets that wrong.

**This does not reopen [[survey-cache-scope]].** That incident was a memo keyed
by probe name alone, so six anchors at six roots were served one another's
candidate tables. Under a predicate the answers still differ by root — because
the predicate differs. What is shared is the part that was never in dispute: a
file's candidates are the same facts no matter who is asking.

## The writer supplies the sort key; the index only sorts by it

`Indexed` carries a `sort` string, and rows come back ordered by `(sort, ord)`.
The index never derives that key from the path.

It cannot, because path order is not string order. `walk` sorts `PathBuf`s,
which compare component by component, and SQL's `ORDER BY rel` compares bytes.
They disagree exactly when a file and a directory share a stem — `b.rs` against
`b/x.rs`, `index.ts` against `index/a.ts`, `mod.rs` against `mod/a.rs` — because
`'.'` is 0x2E and `'/'` is 0x2F, while a component comparison finds `b` a prefix
of `b.rs` and therefore smaller. Modern Rust without `mod.rs`, TypeScript, and
Python all lay out repositories that way.

`report` reads `nth` as an index into the tied candidates, so a backend that
reorders here renames which object an anchor is about while nobody has touched
the code — the failure [[survey-walk]] exists to prevent. The conformance suite
pins the exact pair: `b/x.rs` must come back before `b.rs`, and a backend that
orders by the raw path fails on it before it has a caller.

Handing the key in rather than deriving it also lets an aggregating extractor
order by something that is not a path at all.

## A write reopens a sealed generation

`seal` records when a walk finished. Any `write` after that clears it.

The alternative — a sealed generation that stays sealed while rows underneath it
are being replaced — is a snapshot nobody was promised. Halfway through an
update the index holds some files at their new content and some at their old,
and a query against it can answer `found:false` about a symbol that is in a file
not yet re-read. That is the same lie a partial index tells, arriving through a
different door. A walk that finds nothing changed writes nothing and stays
sealed, so the ordinary case costs nothing.

Content changes do **not** change the generation. That is deliberate and it is
why the seal time has to be carried out to the caller: the contract is bounded
staleness, not freshness, and how stale is a fact the reader is owed rather than
a detail the index keeps to itself.

## When this changes, ask

Does anything derive an ordering from `rel` instead of using the `sort` the
writer supplied? Does any root filter use a prefix test rather than `under`?
Both are silent: the first renames what an anchor watches, the second answers
about the wrong subtree, and neither produces an error anyone would see.

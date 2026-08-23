---
about:
  - domains/coding/cli/src/verbs/anchor.rs#run
  - domains/coding/cli/src/verbs/anchor.rs#declaration
  - domains/coding/cli/src/verbs/anchor.rs#already_declared
watch: [sig, logic]
---

# The front door declares an obligation; it does not create the memory

`gmr anchor <coordinate>` says one thing: **on this fact there ought to be a
memory**. It routes the coordinate, writes an `AnchorDecl` into
`.anchor/anchors.toml`, and opens the anchor. Nothing is written into any
memory store.

An anchor with no memory is `barren`, which [[cli-doctor-run]] never turns
red — the model already had a name for "declared and not yet fulfilled", so
this needs no new state.

## Why the declaration and the memory are separated at all

Where a memory is kept is not GMR's business — it reads through
`ContentProvider`, lists through `MemorySource`, and never writes. A front
door that creates the memory contradicts that for exactly one store, and it
did: whoever kept memories in their own system got a second copy written
into the repository, with GMR watching the copy and nothing watching the
difference. Two copies of one judgement and nothing detecting the drift is
the failure this whole tool exists to report.

Separating them makes every store the same width at the door:

```
declare   gmr anchor <coord>                    → anchors.toml, in the repository
name it   gmr anchor <coord> --record <p>:<id>  → an existing record, wherever it lives
   or     gmr attest <p>:<id> --anchors <coord> → the agent's road; it wrote the record
   or     gmr anchor <coord> -m "…"             → git, where a note is both
```

## `-m` writes a note and no `anchors.toml` entry, and that is not an exception

A git note carries `about:` in its frontmatter, so the file **is** the
memory and the declaration at once — that is the git provider's storage
layout, not a GMR concept. Writing an `anchors.toml` entry beside it would
give one anchor two declarations, and [[cli-sync-run]]'s `merged` would then
have to pick — a reader asking "who declared this anchor" would get two
answers. So the two paths are exclusive by construction, not by care.

## An existing declaration is left alone, in either channel

`already_declared` asks `merged` — anchors.toml *and* notes — not just the
file being appended to. Appending a key a note already declares would shadow
that note's declaration silently, since `merged` prefers the file.

Nothing rewrites a declaration that is there. Re-routing an existing
coordinate is a criteria change, and criteria changes are sealed judgements
(`revise`, `accept --criteria`) — never a side effect of running the front
door twice. This is `write_note`'s "already yours; left alone" applied to
the other channel.

## `--record` records `Adjudicated`, and there is no flag to make it otherwise

A person named both the coordinate and the record, which is the same act
`gmr bind` performs. An agent vouching for a record it wrote itself uses
`gmr attest` — a separate verb precisely so the source cannot be forged by
forgetting a flag ([[cli-bind-run]]). Both go through `bind::assert_on`, so
there is one place that decides what a binding costs to make.

## When this changes, ask

Does a new flag write into a memory store? Then GMR has taken on storing
memories, and the three read-only contracts stop describing what it does.

Does anything start rewriting an existing declaration? That is a criteria
change wearing the front door's clothes, and it escapes `--why`.

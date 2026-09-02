---
about:
  - console/cli/src/verbs/anchor.rs#run
  - console/cli/src/verbs/anchor.rs#declaration
  - console/cli/src/verbs/anchor.rs#already_declared
  - console/cli/src/verbs/anchor.rs#write_note
  - console/cli/src/verbs/anchor.rs#names
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

## A note written under an existing declaration binds and does not declare

The new default invites the crossing: a reader sees "nothing is bound here
yet" and reaches for `-m`. So `write_note` takes whether the coordinate is
already declared, and writes `anchors: ["<coord>"]` instead of `about:` when
it is — the bare-key form [[cli-memories-entry]] describes, which binds
without declaring and which `merged` leaves alone.

The alternative is one anchor with two declarations. Nothing breaks at once:
`merged` prefers the file, so the note's frontmatter simply stops meaning
anything, and a later `shape:` or `watch:` added to it does nothing at all,
silently. `bare-key` does not fire either, because the key *is* declared —
by the file.

`names` therefore reads both spellings when deciding whether an existing
note already owns a slug. Reading only `about:` would make a bind-only note
look like it belongs to some other coordinate, and the next run would write
a second file for the same one.

## An existing declaration is left alone, in either channel

`already_declared` asks `merged` — anchors.toml *and* notes — not just the
file being appended to. Appending a key a note already declares would shadow
that note's declaration silently, since `merged` prefers the file.

Nothing rewrites a declaration that is there. Re-routing an existing
coordinate is a criteria change, and criteria changes are sealed judgements
(`revise`, `accept --criteria`) — never a side effect of running the front
door twice. This is `write_note`'s "already yours; left alone" applied to
the other channel.

## The reminder asks the bindings, it does not infer from this run

Whether the anchor still owes a memory is read back from
`Runtime::bindings_on` after everything this invocation does. Inferring it
from "did I just write something" makes the reminder disappear on the second
run — exactly when the obligation is still outstanding and the reader has
come back. There is no such field in `--json`: `doctor` already answers
"what is barren" for the whole repository, and a second answer beside it is
a second thing to keep true.

## `--record` records `Adjudicated`, and there is no flag to make it otherwise

A person named both the coordinate and the record, which is the same act
`gmr bind` performs. An agent vouching for a record it wrote itself uses
`gmr attest` — a separate verb precisely so the source cannot be forged by
forgetting a flag ([[cli-bind-run]]). Both go through `bind::assert_on`, so
there is one place that decides what a binding costs to make.

## One act, one answer, and the exit code is about the repository

`--json` prints exactly one object. It carries what this run declared, the
binding it made under `bound`, whether the anchor still owes a memory, and
sync's whole report nested under `sync` — reached through
[[cli-sync-run]]'s `synced` so no second answer lands on the same stream.

`bound` and `sync.bound` are different questions and both are true: the
first is the assertion this run made by hand (`Adjudicated`), the second is
what `align_bindings` derived from notes. Reporting only the second is what
told a reader "nothing was bound" in the same breath as binding something —
and a reader who believes that goes back to trusting a memory nothing
vouches for, which is this tool inverted.

The exit code stays sync's: a note elsewhere that names no live anchor makes
the run loud, and it does not stop the record named here from being bound.
They answer different questions — *is this repository sound* and *did this
act happen* — and both answers are in the output, so neither has to be read
off the other.

The address is resolved before anything is written. Resolution is a pure
question ([[cli-address-resolution]]) and answering it first means a typo
costs nothing, rather than leaving a declared anchor behind a refusal.

## When this changes, ask

Does a new flag write into a memory store? Then GMR has taken on storing
memories, and the three read-only contracts stop describing what it does.

Does anything start rewriting an existing declaration? That is a criteria
change wearing the front door's clothes, and it escapes `--why`.

## A URL takes the other branch

`run` splits on whether the coordinate is a URL. A path routes by extension as it
always has; a `http://` or `https://` coordinate is instead *generated into* a
declaration — an `[http.<name>]` probe and an anchor keyed by a short name rather
than by the URL itself. The rule this file states about `.anchor/anchors.toml`
is unchanged and is what governs it: append only, and re-routing a name that is
already taken is a criteria change rather than an overwrite. [[cli-fetched-facts]]
is the whole of that path.

<p align="center">
  <a href="https://github.com/Anchorstate-Lab/GMR">
    <img src="docs/images/GMR%20-%20Grounded%20Memory%20Runtime.png" alt="GMR — Grounded Memory Runtime">
  </a>
</p>

<p align="center">
  <a href="https://github.com/Anchorstate-Lab/GMR/actions/workflows/rust.yml"><img src="https://github.com/Anchorstate-Lab/GMR/actions/workflows/rust.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="https://www.npmjs.com/package/@anchorstate-lab/gmr"><img src="https://img.shields.io/npm/v/%40anchorstate-lab%2Fgmr?logo=npm&color=cb3837" alt="npm"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.88%2B-orange?logo=rust&logoColor=white" alt="Rust 1.88+"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <a href="https://deepwiki.com/Anchorstate-Lab/GMR"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
</p>

> Keep what an agent knows, and what it concludes, attached to the facts underneath.

GMR is an anchoring layer between judgment and changing reality. It keeps a
long-lived memory attached to the facts it depends on and hands it back when
those facts drift; and it holds a one-shot conclusion to the reading it was
built from, so an answer stops being supported the moment its ground moves.

## What it is

An **anchor** is a small state machine watching one coordinate — a function, a
file, a schema field, an HTTP endpoint, a SQL result. A probe observes it, a
rule table interprets the observation, and an append-only journal records every
transition. Two different things bind to an anchor:

```
memory     a long-lived constraint someone wrote and reviewed   fails by drifting
inference  what one analysis concluded for the task in hand     fails by losing its ground
```

A **memory** is not derivable from the facts — that is why somebody had to write
it down. When the anchor moves, the memory is not false, it is *due*: it comes
back for a person to re-read, and accepting it seals a reason into the journal.

An **inference** is a sentence an agent produced this turn. It fails another
way: the reading it rested on moved, or it was never built from that reading at
all. Nobody has to re-read it — the condition its author stated simply stopped
holding.

```
memory     check  →  handed back  →  a person re-reads  →  accept --why
inference  ground →  still holds / built elsewhere / depends broken  →  the caller decides
```

GMR records **structure, not entailment.** It says that a claim is bound to
these anchors, that those anchors read differently now than when it was bound,
that a claim cited a reading its anchor never took, that the invariant its author
wrote no longer holds. It does not say the claim is therefore false — reading a
sentence and judging it is a verdict nobody can recompute, and everything here
is built to be recomputable by a third party from the journal, the store and the
probe.

## Why use it

Use GMR when a judgment can outlive the facts that produced it.

* Code changes — an architectural decision may no longer apply after a refactor
* APIs evolve — an assumption about an interface can become invalid after a contract change
* Configuration and data change — a memory resting on a deployment setting, a price table or a registry entry goes stale silently
* Agents work over long-lived projects — notes accumulated weeks ago drift away from the current codebase, and so do the conclusions built on them

```
Fact at T1                  Fact at T1
   ↓                           ↓
Memory created              Memory + Anchor        Answer + the reading it cited
   ↓                           ↓                            ↓
Fact changes at T2          Anchor sees the change    the ground moved
   ↓                           ↓                            ↓
Memory remains              Memory handed back        the answer no longer stands
   ↓                           ↓                            ↓
Stale memory retrieved      A person re-reads it      re-conclude, from a fresh reading
```

If a judgment would become questionable when some part of the world changes,
that judgment is a candidate for GMR.

## Install

### npm

```sh
npm install -g @anchorstate-lab/gmr
```

The wrapper loads the matching platform bundle. If your operating system or
architecture is not among the published bundles, it prints a fallback message
and points at the install script. The same package is also the Node SDK — see
[Using it from Node](#using-it-from-node).

### Direct install script

```sh
curl -fsSL https://raw.githubusercontent.com/Anchorstate-Lab/GMR/main/dist/install.sh | sh
```

If you are using a fork or mirror, replace `Anchorstate-Lab/GMR` with your repository path.

### Build from source

```sh
cargo install --path domains/coding/cli --locked --root ~/.local
```

This installs the assembled `gmr` CLI built from the current workspace.

## Quick start

GMR works against a project directory, including repositories with no Rust in
them. Run commands with `--repo <path>` (default is `.`).

### 1. Initialize

```sh
gmr init
```

This creates the local `.anchor/` layout, installs the built-in probe bundle,
and (the first time only) writes a Claude Code skill doc to
`.claude/skills/gmr/SKILL.md` — pass `--global` to write it to
`~/.claude/skills/gmr/SKILL.md` instead. It does not create anchors or notes
for you. Safe to rerun; it never overwrites a file that already exists.

### 2. Anchor a coordinate, then name the memory

```sh
gmr anchor src/auth.ts#createSession
```

`anchor` routes the coordinate to a probe, a shape and a position, declares it
in `.anchor/anchors.toml`, and opens it. It puts no memory anywhere: the anchor
says *there ought to be a memory about this*, and `doctor` reports it as
`barren` until one is bound.

Then say which memory, whichever way you keep them:

```sh
# it already exists, in your own memory system — nothing is copied here
gmr anchor src/auth.ts#createSession --record claude-code:session-expiry.md

# you keep no memories of your own, so GMR writes one here as a note
gmr anchor src/auth.ts#createSession -m "sessions expire after 30 minutes because ..."
```

Those are two ways to say *which memory*, not two kinds of anchor — everything
downstream is identical. A note is the one case where the memory and the
declaration are the same file, which is why `-m` writes no `anchors.toml`
entry. You can also hand-write that note yourself and run `gmr sync` to open
it:

```md
---
about: src/auth.ts#createSession
watch: [sig, logic]
---

# Sessions must still be created inside the service boundary
```

Run `gmr anchor` with no coordinate to open whatever the declarations and
notes already ask for — what a fresh clone needs, since the journal doesn't
travel with the repository.

### 3. Check for drift

```sh
gmr check
```

```
src/auth.ts#createSession   signature-changed
  → auth-createSession

1 of 1 handed a memory back. Re-read it: does what you wrote still hold?
```

`check` evaluates due anchors and exits 1 if any axis a note asked about
moved, handing back the notes bound to what moved. It also flags anchors
whose declaration no longer matches their live criteria, and anchors
standing on a reading a different probe instrument took — resolve those
(see `accept --criteria` and `rebase` below) before trusting a quiet result.

Reformatting does not count as a change. A signature and a body are read from
the parse tree, not from the source text, so rewrapping a parameter list,
adding a trailing comma, re-indenting or writing a comment leaves every anchor
where it was. Run your formatter freely; what `check` reports is something a
compiler would also see.

### 4. Accept what you find

```sh
gmr accept src/auth.ts#createSession --why "..."
```

You looked, and what `check` showed is the new baseline; `accept` clears the
vector and seals the reason permanently into the journal. If a declaration
change is pending too, pass `--baseline` or `--criteria` explicitly — they're
separate judgments and don't share one reason.

### 5. Record a conclusion, and ask later whether it still stands

The loop above is for memories a person maintains. A conclusion an agent
reaches in one sitting gets its own pair of verbs.

```sh
# read the anchor, and get the address of the exact reading it hands you
gmr read src/auth.ts#createSession --json     # → "fact_address": "1efbd854…"

# record what you concluded from that reading, and what keeps it true
gmr said "createSession returns a 30-minute ttl" \
  --on src/auth.ts#createSession \
  --saw 1efbd854… \
  --depends 'all(anchors, not state.v.sig)'

gmr standing
```

```
said:20260830T205425  createSession returns a 30-minute ttl
    src/auth.ts#createSession   the ground moved since this was bound: now.body · now.sig · v.logic · v.sig   saw its reading at 1
    depends: no longer holds

1 conclusion(s) · 1 the ground no longer settles · 0 built beside an anchor rather than through it · 0 that cited no reading at all
```

`standing` exits 1 when a conclusion's ground no longer settles it, or when one
cites a reading no anchor ever took — the shape of an answer computed *beside*
an anchor rather than through it. Build the answer from what `read` returned and
cite the address it came with, and the delivery path and the anchor are one look
at the world instead of two.

`--saw` and `--depends` are both optional and reported separately when absent: a
conclusion that vouched for nothing is counted, not assumed to be fine.
`--depends` is one expression over the anchors the claim names — `all(...)`,
`any(...)`, `count(...)`, with `state` bound to each anchor in turn — and saying
the narrowest true thing is what buys you something: a finding about a signature
survives an edit to the body, which a whole-state comparison cannot express.

`gmr standing <id> --retire` stops asking about one. What it said stays in the
table; nothing reads it again.

### 6. See everything being watched

```sh
gmr status --json
```

`status` reports every anchor, its axes, and the notes bound to it. Reads only.
`gmr doctor` is the health report over the whole corpus — anchors nobody bound a
note to, records whose store no longer has them, notes with lint problems — and
`gmr health` reports whether each anchor is pointed anywhere useful: how often it
fired, and how often a hand-back actually ended in a memory being rewritten.

### 7. Read the whole graph at once

```sh
gmr atlas
```

`atlas` writes every anchor, every memory and what binds them to
`.anchor/output/atlas.html` — one self-contained file that opens straight from
disk, with nothing fetched over the network. The rail on the left is the
repository tree, the middle is the anchor–memory graph, and the right panel is
the memory text for whatever you pick in either. Colour says how loudly
something is asking to be looked at, so a branch you have collapsed still shows
that something under it moved. `--out <path>` writes it elsewhere.

Anchors and memories are many-to-many — one memory can be about nine
coordinates, one coordinate can carry three memories — and that is the shape
this page exists to make readable.

### 8. Memories that are not files in this repository

`memories/*.md` is one store among several, not the only shape a memory can
arrive in. GMR keeps the binding — which anchors a record is about — in its own
table beside your store, and never writes into the store itself.

```sh
export MEM0_API_KEY=... MEM0_USER_ID=...
gmr memories --provider mem0
gmr bind <memory-uuid> --provider mem0 --anchors src/auth.ts#createSession
```

`memories` lists what a store will show and which of it is already bound —
reach for it because a uuid is not something you can guess. The uuid is the
whole reference, and mem0 keeps it stable when it rewrites a memory in place,
so `gmr check` reports that rewrite the same way it reports a moved function.

A store nobody compiled in is declared in `.anchor/providers.toml`, naming a
`fetch` script and optionally a `list` one; `gmr doctor` then says what that
store can and cannot do here rather than leaving you to find out by watching it
fail.

What each store owes is one contract; history and listing are capabilities a
store either has or does not. Claude Code's own memory files, for instance,
keep no history — so a rewritten record is still reported, you just re-read the
whole thing instead of a diff. A store that will **not answer** and a record
that is **gone** are reported as different things and only the second turns a
build red: nobody holding this repository can fix somebody else's outage.

## Common commands

The front door — the ten verbs `gmr --help` shows:

- `init` — set up `.anchor/`, install bundled probes, write the skill doc
- `anchor` — watch a coordinate and name the memory that goes with it
- `status` — what is being watched, on which axes, with which memories
- `check` — did anything move on an axis a memory asked about?
- `accept` — take what an anchor now shows as the new baseline, or take a
  changed declaration's criteria (`--baseline` / `--criteria` / `--all --criteria`)
- `said` — record one conclusion, the readings it was built from, and the
  invariant that keeps it standing
- `standing` — do the recorded conclusions still hold? `--retire` stops asking
  about one
- `atlas` — write the whole anchor–memory graph as one HTML page
- `close` — retire an anchor permanently
- `memories` — what each memory store here will show you, and which of it is
  already bound. On the front door because when your memories are not files,
  it is the only way to find a reference to bind: a mem0 uuid is not something
  you can guess at.

Everything else still works, reachable through `gmr help <name>`:

- `probes list` / `probes build` — list or build available probes
- `sync` — open anchors declared by notes and align bindings (what `anchor`
  with no coordinate runs)
- `open` — create an anchor directly by hand
- `observe` / `pass` / `read` — evaluate one anchor, run a batch, or read anchor
  state and the address of the reading it hands back (`--fresher-than-secs`
  looks again first)
- `reprobe` / `retransition` / `reterminal` / `rebase` / `restate` — hand-drive
  one part of an anchor's criteria; each needs `--why`, sealed into the journal
- `bind` / `attest` / `reaffirm` / `cobound` — manage reference bindings;
  `attest` is how an agent says a record it just wrote is about these anchors
- `link` — record a relationship between references
- `edges` — read journal transitions since a point
- `health` — per-anchor liveness, and whether the anchor is aimed anywhere
- `doctor` — anchors never observed, with no note, unresolvable, records whose
  store lost them, and notes with lint problems
- `requeue` — force an anchor back onto the due queue
- `publish` — install a directory as a named probe artifact
- `export` / `import` — snapshot and replay store contents

Most commands support `--repo <path>` and `--json`.

## Using it from Node

The npm package is both the CLI and a native addon. `open()` returns a handle
onto the same store the CLI uses — point both at one repository and they are two
writers on one journal, not two half-stories.

```js
const { open, CONTRACT } = require("@anchorstate-lab/gmr");

const gmr = await open({ root: "/path/to/project" });

const reading = await gmr.sample("src/auth.ts#createSession");
// build the answer from reading.facts, then cite what you were shown
await gmr.bind("said:2026-08-30-ttl", ["src/auth.ts#createSession"], "self_attested", {
  saw: [reading.fact_address],
  asserts: { text: "sessions expire after 30 minutes" },
  depends: "all(anchors, not state.v.sig)",
});

const [standing] = await gmr.ground(["said:2026-08-30-ttl"]);
```

Seven verbs cross the boundary: `sample`, `ground`, `since`, `bind`, `revoke`,
`open`, `close`. Deliberately absent are the scheduling verb (`pass` — whichever
process runs the observation loop owns the cadence, not every caller) and
anything that changes criteria (an owner's judgement, made in a reviewed commit).
A deployment that only calls `since` will see nothing change unless something is
observing: either pass `max_staleness_ms` so `ground` and `sample` look while
they answer, or run `gmr pass` on a schedule.

`CONTRACT` is the version of the shapes a caller may match on
(`index.d.ts` declares them, and the build fails if a contract type changes shape
while the string stands still).

Probes are declared in the call — `recipes` for `http`, `file` and `sql` probes,
`scripts` for shell ones — so a caller with no repository and no Rust can still
say what a probe is.

## Build and verify

If you are working in this repository and want to build or verify it:

```sh
cargo build --release -p coding-anchor
cargo test --workspace
sh gate.sh
sh acceptance.sh
```

## Documentation

- `docs/ARCHITECTURE.md` — architecture and design source of truth, written out as one long argument
- `docs/architect.md` — repository layering and package responsibilities
- `CLAUDE.md` — design decisions and repository norms
- `memories/` — this repository's own notes, anchored to its own code

## License

[MIT](LICENSE) © 2026 Zongming-He

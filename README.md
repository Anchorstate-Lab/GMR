<p align="center">
  <a href="https://github.com/Anchorstate-Lab/GMR">
    <img src="docs/images/GMR%20-%20Grounded%20Memory%20Runtime.png" alt="GMR — Grounded Memory Runtime">
  </a>
</p>

> Keep agent memory grounded in the facts it depends on.

GMR is a grounding layer between agent memory and changing reality. It keeps memories attached to the facts they depend on, detects when those facts drift, and surfaces affected memories before they become stale assumptions.

## What it is

GMR turns a memory into a grounded, observable relationship.

Instead of storing a note as an isolated piece of information, GMR records:

* What the memory is about — the code, interface, configuration, or other observable target
* What to watch — the properties or changes that matter to the memory
* How to observe it — a reproducible probe with a known version
* What changed — a journaled transition from the previous state

When the observed state crosses a declared transition, the anchor moves and GMR returns the memories bound to it.

In other words:

```
Memory
  ↓
"What does this depend on?"
  ↓
Anchor
  ↓
"What should we watch?"
  ↓
Observation
  ↓
"Did it change?"
  ↓
Surface the memory
```

GMR does not store a copy of the world or decide whether a judgment is correct. It maintains the relationship between a judgment and the observable state it depends on.

## Why use it

Use GMR when your agent’s memories can outlive the facts that created them.

This matters when:

* Code changes — an architectural decision may no longer apply after a refactor
* APIs evolve — an assumption about an interface can become invalid after a contract change
* Configuration changes — a memory based on a deployment or runtime setting can become stale
* Behavior changes — an observed system behavior may no longer match the reason behind an old decision
* Agents work over long-lived projects — memories accumulated weeks or months ago may silently drift away from the current codebase

Without GMR:
``` 
Fact at T1
   ↓
Memory created
   ↓
Fact changes at T2
   ↓
Memory remains
   ↓
Agent retrieves stale memory
```
With GMR:
```
Fact at T1
   ↓
Memory + Anchor
   ↓
Fact changes at T2
   ↓
Anchor detects the change <-
   ↓
Memory is surfaced again
   ↓
Agent re-evaluates it
```
The key use case is simple:

If a memory would become questionable when some part of the world changes, that memory is a good candidate for GMR.

## Install

### npm (recommended for end users)

If the npm packages are published, install the wrapper package:

```sh
npm install -g @anchorstate-lab/gmr
```

The npm wrapper will load the matching platform bundle, if available.
If your operating system or architecture is not supported by the published
bundle, the wrapper prints a fallback message and points you to the direct
installation script.

### Direct install script

The repository includes a simple install script for prebuilt releases.
If you want a direct binary install without npm:

```sh
curl -fsSL https://raw.githubusercontent.com/Anchorstate-Lab/GMR/main/dist/install.sh | sh
```

If you are using a fork or mirror, replace `Anchorstate-Lab/GMR` with your repository path.

### Build from source

If you want to build the CLI yourself from this repository:

```sh
cargo install --path domains/coding/cli --locked --root ~/.local
```

This installs the assembled `gmr` CLI built from the current workspace.

## Quick start

GMR works against a project directory, including repositories with no Rust in them.
Run commands with `--repo <path>` (default is `.`).

### 1. Initialize

```sh
gmr --repo /path/to/project init
```

This creates the local `.anchor/` layout, installs the built-in probe bundle,
and (the first time only) writes a Claude Code skill doc to
`.claude/skills/gmr/SKILL.md` — pass `--global` to write it to
`~/.claude/skills/gmr/SKILL.md` instead. It does not create anchors or notes
for you. Safe to rerun; it never overwrites a file that already exists.

### 2. Anchor a coordinate and write the memory

```sh
gmr --repo /path/to/project anchor src/auth.ts#createSession \
  -m "sessions expire after 30 minutes because ..."
```

`anchor` routes the coordinate to a probe, a shape and a position, writes a
note under `memories/` with that memory, opens the anchor, and binds the note
to it — all in one step. Omit `-m` and the note is left for you to write,
reported as `unwritten` until you do. Equivalently, you can hand-write the
note yourself with the same frontmatter and run `gmr sync` to open it:

```md
---
about: src/auth.ts#createSession
---

# Sessions must still be created inside the service boundary
```

Run `gmr anchor` with no coordinate to open whatever the declarations and
notes already ask for — what a fresh clone needs, since the journal doesn't
travel with the repository.

### 3. Check for drift

```sh
gmr --repo /path/to/project check --json
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
gmr --repo /path/to/project accept src/auth.ts#createSession --why "..."
```

You looked, and what `check` showed is the new baseline; `accept` clears the
vector and seals the reason permanently into the journal. If a declaration
change is pending too, pass `--baseline` or `--criteria` explicitly — they're
separate judgments and don't share one reason.

### 5. See everything being watched

```sh
gmr --repo /path/to/project status --json
```

`status` reports every anchor, its axes, and the notes bound to it. Reads only.

### 6. Read the whole graph at once

```sh
gmr --repo /path/to/project atlas
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

## Common commands

The front door — seven verbs `gmr --help` shows:

- `init` — set up `.anchor/`, install bundled probes, write the skill doc
- `anchor` — watch a coordinate and write the memory that goes with it
- `status` — what is being watched, on which axes, with which memories
- `check` — did anything move on an axis a memory asked about?
- `accept` — take what an anchor now shows as the new baseline, or take a
  changed declaration's criteria (`--baseline` / `--criteria` / `--all --criteria`)
- `atlas` — write the whole anchor–memory graph as one HTML page
- `close` — retire an anchor permanently

Everything else still works, reachable through `gmr help <name>`:

- `probes list` / `probes build` — list or build available probes
- `sync` — open anchors declared by notes and align bindings (what `anchor`
  with no coordinate runs)
- `open` — create an anchor directly by hand
- `observe` / `pass` / `read` — evaluate due anchors, return moved notes, or
  read raw anchor state
- `reprobe` / `retransition` / `reterminal` / `rebase` / `restate` — hand-drive
  one part of an anchor's criteria; each needs `--why`, sealed into the journal
- `bind` / `reaffirm` / `cobound` — manage reference bindings
- `link` — record a relationship between references
- `edges` — read journal transitions since a point
- `health` — inspect anchor liveness
- `doctor` — anchors never observed, with no note, unresolvable, or notes
  with lint problems (`unclaimed`, `bare-key`, `long-hand`, `retired`)
- `requeue` — force an anchor back onto the due queue
- `export` / `import` — snapshot and replay store contents

Most commands support `--repo <path>` and `--json`.

## Build and verify

If you are working in this repository and want to build or verify it:

```sh
cargo build --release -p coding-anchor
cargo test --workspace
sh gate.sh
sh acceptance.sh
```

## Documentation

- `docs/GMR.md` — architecture and design source of truth
- `docs/architect.md` — repository layering and package responsibilities
- `CLAUDE.md` — design decisions and repository norms
- `memories/` — repository notes and grounded records

## License

[MIT](LICENSE) © 2026 Zongming-He

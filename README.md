# GMR — Grounded Memory Runtime

> Track subjective judgments by anchoring them to recomputable observations in your repository.

GMR is a lightweight runtime for binding notes to facts, replaying the observation
that generated them, and warning you when the world moves along a direction you
declared.

## What it is

GMR is a CLI tool that helps you:

- attach a judgment or note to an observable part of a repository
- keep that judgment bound to a reproducible observation
- detect when the relevant world state has changed
- return the notes bound to anchors that moved

It is not a linter or rule engine. It is a runtime for anchoring notes to
observable state and preserving that binding over time.

## Why use it

Use GMR when you want to make a manual judgment accountable:

- a contract should still hold after a refactor
- a behavioural assumption should still be true after code changes
- a note should be surfaced again when the thing it depends on moves

GMR keeps the judgment attached to the fact it depends on and reports if that
fact changes.

## Install

### npm (recommended for end users)

If the npm packages are published, install the wrapper package:

```sh
npm install -g @zongming_he/gmr
```

The npm wrapper will load the matching platform bundle, if available.
If your operating system or architecture is not supported by the published
bundle, the wrapper prints a fallback message and points you to the direct
installation script.

### Direct install script

The repository includes a simple install script for prebuilt releases.
If you want a direct binary install without npm:

```sh
curl -fsSL https://raw.githubusercontent.com/Zongming-He/GMR/main/dist/install.sh | sh
```

If you are using a fork or mirror, replace `Zongming-He/GMR` with your repository path.

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

## Common commands

The front door — six verbs `gmr --help` shows:

- `init` — set up `.anchor/`, install bundled probes, write the skill doc
- `anchor` — watch a coordinate and write the memory that goes with it
- `status` — what is being watched, on which axes, with which memories
- `check` — did anything move on an axis a memory asked about?
- `accept` — take what an anchor now shows as the new baseline, or take a
  changed declaration's criteria (`--baseline` / `--criteria` / `--all --criteria`)
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

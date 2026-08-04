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
npm install -g @gmr/cli
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

This creates the local `.anchor/` layout and installs the built-in probe bundle.
It does not create anchors or notes for you.

### 2. Write a note

Create a note file in your project with frontmatter that identifies the thing you
want to track.

Example:

```md
---
about: src/auth.ts#createSession
---

# Sessions must still be created inside the service boundary
```

### 3. Open the note

```sh
gmr --repo /path/to/project sync
```

`sync` opens the anchors that notes declare and binds those notes to the
corresponding anchors.

### 4. Observe changes

```sh
gmr --repo /path/to/project observe
```

If the observable state behind an anchor has changed, `observe` exits with code
1.

### 5. See moved notes

```sh
gmr --repo /path/to/project pass --json
```

`pass` reports the notes bound to anchors that moved, so a consumer can act on them.

## Common commands

- `init` — set up `.anchor/` and install bundled probes
- `probes list` — list available probes
- `sync` — open anchors declared by notes and align bindings
- `open` — create an anchor directly by hand
- `observe` — evaluate due anchors and detect movement
- `pass` — return moved notes and bound anchors
- `read` — inspect current anchor state
- `reprobe` — change which probe an anchor uses
- `retransition` — update transition rules for an anchor
- `reterminal` — update a terminal status set
- `rebase` — recapture anchors after probe or engine changes
- `restate` — adjust anchor state with sealed rationale
- `bind` / `reaffirm` / `cobound` — manage reference bindings
- `link` — record a relationship between references
- `close` — retire an anchor permanently
- `edges` — read journal transitions since a point
- `health` — inspect anchor liveness
- `doctor` — find anchors that were never observed or have no note
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

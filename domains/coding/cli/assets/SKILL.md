---
name: gmr
description: Use in any repository containing a `.anchor/` directory. GMR grounds subjective notes to recomputable facts about the code and tells you when they've drifted. Trigger whenever `.anchor/anchors.toml` exists at the repo root, and especially before trusting an old note, comment, or doc that claims something about specific code is still true.
---

# gmr — grounded memory runtime

## What this is

An anchor is a small state machine watching one coordinate in the codebase (a function, a file, a schema field — whatever a probe can read). A note is a subjective record — yours — bound to one or more anchors. `gmr` doesn't store the note's content; it stores that the note is *about* this anchor, and what the anchor looked like when the note was written. When the anchor's state moves, the note is no longer guaranteed current — `gmr` tells you that, it doesn't guess whether the note still holds.

## Detection

If `.anchor/anchors.toml` exists at the repo root, this repository is managed by `gmr`. Before trusting any note, comment, or memory that claims something about the code is still true, check whether it's grounded and whether it drifted:

```
gmr doctor --json
gmr read --json
gmr observe --json
```

## The loop

This is a loop you run yourself as part of normal work — not a human-only ritual you wait to be asked for:

1. `gmr init` — creates `.anchor/`, installs bundled probes. Idempotent, safe to rerun.
2. Write a note under `memories/<name>.md` naming what it's about:
   ```
   ---
   about: src/auth.ts#createSession
   ---
   Sessions expire after 30 minutes because ...
   ```
3. `gmr sync` — opens anchors for every coordinate declared in `.anchor/anchors.toml` or named by a note under `memories/`, and binds the notes to them. This is automatic: naming a coordinate in a note is enough to open an anchor for it on the next `sync`. Use `gmr sync --dry-run` first if you want to preview what would open or bind before committing to it.
4. `gmr observe` (or `gmr pass` for only what's due) — asks whether the world still matches. Exits 1 if anything moved.
5. `gmr read --json` — each anchor's current state, for scripting.

## What's worth anchoring — judgment, not a rule

`gmr` ships no fixed list of "anchor this, not that," and this document doesn't add one either. The heuristic worth carrying in your head:

- A fact that fully decides the judgment on its own → don't anchor it, just check it directly.
- A fact that doesn't constrain the judgment at all → anchoring it won't hold; it degenerates into prose storage with extra ceremony.
- A fact that constrains the judgment but doesn't decide it → that's what anchoring is for.

Apply this yourself, in context, the same way you'd decide whether a comment is worth writing. Nobody — including this document — should be deciding it mechanically on your behalf.

## Verbs that seal a reason (`--why`)

`reprobe`, `retransition`, `reterminal`, `rebase`, `restate`, and `close` all require `--why`, and it's sealed permanently into the journal. These are judgment calls being revised, not routine writes — give the real reason. It gets read later, by you or someone else, trying to reconstruct why the criteria changed. A terminal anchor cannot be un-terminaled; correcting a bad judgment means opening a new anchor with `--supersedes`, not fighting the old one.

## Reading `gmr doctor --json`

```json
{"anchors": N, "live": N, "absent": [...], "unseen": [...], "barren": [...], "stranded": [...], "content_versioning": bool, "provider_warnings": [{"provider": "...", "message": "..."}]}
```

- `barren` — anchors nobody has bound a note to yet.
- `absent` — the probe ran and found nothing there. Normal when criteria were written before the code exists — don't read this as "it used to be there and now it's gone" without checking.
- `unseen` — outstanding failed attempts; check the probe or its credentials.
- `stranded` — the declared probe has no installed artifact on this machine (`gmr probes build`).
- `provider_warnings` — a content provider this binary tried to register at startup but couldn't (for example `claude-code` when `$HOME` isn't set). Bindings through it will fail with "no content provider could version" until the underlying cause is fixed. Check this before assuming a failed `gmr bind --provider ...` means the provider name was wrong.

## Binding non-git content

`gmr bind <path> --anchors <key> --provider <name>` binds arbitrary content to an anchor, not only files inside this repo's own git tree. `--provider` defaults to `git`. Whether other providers (for example a `claude-code` provider reading this agent's own memory files) are available depends on how this particular binary was built — `gmr bind --help` reflects what's actually compiled in.

## Don't

- Don't hand-edit `.anchor/anchors.toml`, `.anchor/probes.toml`, or anything under `.anchor/state/` — go through the verbs so the journal stays the one source of truth.
- Don't retry a failed transition condition expecting it to eventually succeed — an eval failure means the rule is wrong, not that the world is slow. Fix the rule.

---
about: domains/coding/cli/src/verbs/sync.rs#DEFAULT_FILE
watch: [sig]
---

# A missing declarations file means two different things depending on why it's missing

`DEFAULT_FILE` (`.anchor/anchors.toml`) is optional: `gmr init` never
writes it, so a repository whose every anchor comes from note frontmatter
legitimately has none. `read_declared` treats a missing file at the
*default* path as "nothing declared here" (`Ok(Declared::default())`),
but a missing file at a path the *user explicitly named* (`--file`) is an
error — that is far more likely to be a typo than a deliberate choice, and
staying silent about it would let a mistyped `--file` sync as if nothing
was declared at all.

## When this changes, ask

Does a new default path get the same "missing is fine" treatment
regardless of whether it came from a flag the user set? Only the
default's own absence is unremarkable; an explicit path's absence should
keep failing loudly.

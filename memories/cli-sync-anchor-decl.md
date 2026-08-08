---
about: domains/coding/cli/src/verbs/sync.rs#AnchorDecl
watch: [sig]
---

# What survives an engine upgrade unchanged, and what expands at sync time

`probe` is a name, never a version (see [[cli-rules-probe]]) — a
declaration has to keep meaning the same thing across an engine upgrade
that changes what the probe actually resolves to.

`shape` and `rules` are mutually exclusive: `shape` names a preset that
gets expanded into literal rules at sync time (in `to_transitions`),
specifically so that whatever ends up in the anchor's declaration hash is
the *full* criteria table, not a preset name that could quietly change
meaning if the preset's own rules were edited later without touching this
declaration.

`retain_full` and `cadence_secs` both feed `RunSettings` (via `settings()`)
rather than the sealed criteria — see [[anchor-RunSettings]] for why
"how it runs" stays mutable and outside the hash `shape`/`rules` end up
inside.

## When this changes, ask

Does a new field belong in the expanded transitions (and therefore the
criteria hash), or is it an operating knob like `retain_full`/
`cadence_secs`? Only the former needs the shape-expansion treatment
`shape` gets.

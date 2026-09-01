---
about:
  - console/cli/src/verbs/sync.rs#AnchorDecl
  - console/cli/src/verbs/sync.rs#declare
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

`settings` is a flattened `settings::Declared` — all three operating knobs,
each an `Option` — and feeds `RunSettings` rather than the sealed criteria; see
[[anchor-RunSettings]] for why "how it runs" stays mutable and outside the hash
`shape`/`rules` end up inside.

All three live in one place because a knob this struct cannot spell is a knob
`sync` resets: a declaration is a partial statement, and it needs a partial type
to stay one. [[cli-settings-declared]] carries that reasoning, because it is
about how a declaration is *read* rather than about what this struct holds.

`params` defaults to what routing a coordinate answers rather than to `{}`, so
the same coordinate reaching here through `about:` and through `gmr open`
produces one `ProbeRef` instead of two that differ only in a default nobody
chose.

## `declare` writes this struct, and writes only the part a coordinate settles

`declare` is the other half of `read_declared`, and it lives beside it so one
place knows the file's shape. It writes `key` / `probe` / `shape` / `params` /
`position` and nothing else: `rules`, `terminal` and `settings` are what
somebody states by hand, and `gmr anchor` has none of them to state. A partial
statement is what this struct is for, so writing a partial one needs no second
type.

It **appends** and never rewrites an entry already there. Re-routing a
coordinate that is already declared is a criteria revision — it belongs to
`revise` / `accept --criteria`, where the reason gets sealed — and must never
be a side effect of running the front door twice ([[cli-anchor-declares]]).
Appending also leaves whatever a person wrote around it byte-for-byte intact,
which a full re-serialisation would not.

Field order in the written block is load-bearing, not style: TOML puts a table
under whichever array entry precedes it, so the scalars are emitted before
`params` and `position`. Emit one scalar after them and it lands inside the
previous table — silently repointing an anchor at another coordinate.

## When this changes, ask

Does a new field belong in the expanded transitions (and therefore the
criteria hash), or is it an operating knob like the three inside `settings`?
Only the former needs the shape-expansion treatment `shape` gets.

Does a new operating knob get a field of its own here rather than joining
`Declared`? A knob that only one of the two grids can express is the shape that
just came out, and it presents as sync quietly undoing somebody's tuning.

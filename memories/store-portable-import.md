---
about: crates/gmr-store/src/sqlite/portable.rs#import_jsonl
watch: [sig, logic]
---

# Import only ever replays into a store that is provably empty, and atomically

`import_jsonl` counts every one of the six tables before touching
anything, and refuses outright if any of them has existing rows. Replaying
recreates history at the exact `seq` values the export recorded (see
[[store-portable-expect-seq]]), and that only produces the right history
when nothing already occupies those `seq` slots — importing into a
non-empty store would either collide with existing rows or silently
renumber the replayed ones. The whole import runs inside one transaction
for the same reason a partial success would be worse than a clean failure:
a bad line anywhere has to leave the store exactly as empty as it started,
not half-populated with an export that never finished landing.

Inside the loop, a `BindingAnchors` row's `seq` is trusted as-is, with no
`expect_seq` check of its own — that is safe specifically because the
`Bindings` arm immediately above it already proved (via `expect_seq`) that
the row it names landed at exactly that `seq`. Trusting it a second time
would be redundant, not safer.

## When this changes, ask

Does the pre-flight emptiness check still cover every table a replayed row
could reference? And if `BindingAnchors`' ordering relative to `Bindings`
ever changes, does the "already proved above" trust it relies on still
hold?

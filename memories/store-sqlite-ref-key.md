---
about: crates/gmr-store/src/sqlite/mod.rs#ref_key
watch: [sig, logic]
---

# `ref_key` can `expect()` past the depth guard because `Ref`'s shape is fixed

`canonicalize` can refuse input that nests deeper than its guard allows,
but `ref_key` calls `.expect()` on that result without a fallback, because
`Ref` is two flat string fields — its nesting depth is fixed by the type
itself, not by any data it could ever hold, so the depth guard can never
actually trigger here.

## When this changes, ask

Does `Ref` (or whatever type flows through here) gain a field that could
nest arbitrarily, rather than staying flat by construction? If so this
`expect()` stops being provably safe and needs a real error path.

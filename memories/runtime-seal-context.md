---
about: crates/gmr-runtime/src/seal_context.rs#base
watch: [sig, logic]
---

# Only the part `revise` and `close` actually share lives in `base`

`revise` and `close` each seal a context object that is not the same
shape — they need different fields beyond the three here. `base` exists
specifically as the intersection, not as a template either extends
uniformly; each caller builds its own full context by extending this with
whatever else it needs, rather than `base` trying to grow parameters to
cover both.

## When this changes, ask

Does the new field actually belong to both `revise` and `close`'s
contexts, or only one? Only a field both callers genuinely need belongs in
`base` — otherwise `revise` and `close` should extend it independently.

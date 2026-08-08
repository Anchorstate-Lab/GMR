---
about: crates/gmr-runtime/src/memory.rs#bind
watch: [sig, logic]
---

# `bind` computes `bound_at_seq` here, because only the runtime can fold the log

`MemoryLens::bind` is where `bound_at_seq` (see [[store-binding-record]])
actually gets its value: when `binding.anchors` names exactly one anchor,
it folds that anchor's own log to get its current head (`s.head`) and
stamps that as the seq the binding was made at, so a later read can tell
whether the anchor has moved since. `gmr-store` itself cannot compute this
— it has no access to `fold` or the log, only to whatever `bound_at_seq` it
is handed. For any other anchor count the value stays `None`, for the same
reason `BindingRecord` documents: there is no single head to name.

## When this changes, ask

Does the new binding path still fold the named anchor's own entries to get
`bound_at_seq`, or does it compute a seq some other way (a global counter,
the store's own clock)? The value only means "this anchor's head at bind
time" if it comes from folding that anchor's log specifically.

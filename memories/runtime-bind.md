---
about: crates/gmr-runtime/src/memory.rs#bind
watch: [sig, logic]
---

# `bind` dates a binding against the log, not against an anchor

`MemoryLens::bind` is where `bound_at_seq` (see [[store-binding-record]])
gets its value: `log.head()`, the journal's position at the moment of
binding. `gmr-store` cannot compute it — it has no access to the log, only
to whatever `bound_at_seq` it is handed.

It used to fold the named anchor's own entries for `s.head`, and stamped
`None` whenever the binding named anything other than exactly one anchor,
on the grounds that "which anchor's head would this be" has no answer.
**The question was the wrong one.** `journal.seq` is one global
`AUTOINCREMENT` counter with `anchor` as a column, so one number dates a
binding against any number of anchors: nothing that happened after this
point had happened when we bound.

The two agree wherever the old one was defined — an anchor's `moved_at` is
never above its own head, which is never above the log's head at the same
moment, and every later move takes a higher seq. What changed is that the
value now exists for the case provenance is made of: one memory resting on
several facts.

## The source and the clock arrive from outside

`bind` takes `source` and `at` rather than deciding either. The source is
the caller's fact — `sync` reaching a coordinate through a note is
`Derived`, a person naming one at the CLI is `Adjudicated` — and the runtime
has no way to tell them apart from here. The clock is passed for the reason
[[store-binding-record]] gives: replay should put back the moment the
assertion was made, not the moment it was read back.

## When this changes, ask

Does `journal.seq` stop being one global counter — per-anchor sequences, a
sharded log, a store that renumbers on import? Then one stamp no longer
dates a binding against every anchor it names, and the seq belongs on
`binding_anchors` instead, one per pair.

Does anything start comparing `bound_at_seq` against an anchor's `head`
rather than its `moved_at` (see [[runtime-moved-at]])? The head advances on
entries that are not the world moving.

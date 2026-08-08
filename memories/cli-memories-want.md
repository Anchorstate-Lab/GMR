---
about: domains/coding/cli/src/memories.rs#Want
watch: [sig]
---

# `Want` separates "bind to this key" from "and here is its full declaration"

A note's frontmatter can name an anchor two ways — a bare key
(`Want::Existing`) or a full declaration (`Want::Declared`, built by
`from_about`/`from_spec`). `Want` exists so `scan`'s caller can act on both
uniformly (get the key, decide whether to open/declare it) without caring
which form the note used, while still being able to tell them apart when
it matters — declaring is a stronger claim than binding, and only a
`Declared` want carries enough information to actually open an anchor.

## When this changes, ask

Does a new variant blur the line between "this note binds to a key" and
"this note supplies enough to declare the anchor itself"? Callers that
only need the key (`Want::key`) should not have to care, but callers that
open anchors from notes need this distinction intact.

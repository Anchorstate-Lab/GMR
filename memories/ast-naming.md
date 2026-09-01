---
about: packs/coding/extract/src/ast.rs#naming
watch: [sig, logic]
---

# `name` is only for the kinds that can be pointed at on their own

When a person writes the coordinate `path#name`, they always mean "the **one**
thing in this file called that". So `name` is reserved for the candidates that can
be pointed at on their own — function · module · type. The other two classes each
get their own key:

- `callee`: **mentions** a name rather than introducing it — `call` · `import`
- `member`: is part of some type, and its identity has to carry the owner —
  `field`. The identity of the field `reason` is `Attempt::reason`, not `reason`

What happens without that split was measured. `crates/gmr-core/src/journal.rs#fold`
had **8 candidates** at the time and anchored onto a call site; `#reason` anchored
onto the struct field of that name instead of `fn reason`. Both reported
`exact=true`, `contract`'s missing rule could not catch it, and `status` showed
normal the whole way. Which one `nth=0` picks depends on **traversal order** — that
is, what the anchor watches depends on how tree-sitter walks the tree.

**Nothing was lost.** `{file, kind: "call"}` still lists every call site,
`{file, member: "reason"}` still points at that field. They simply no longer get
to tie with definitions.

## When this changes, ask

A new arm is added to this match → the question is not "does it look like a
definition", it is this one: **can it be pointed at, on its own, by one name in the
file?** Yes → `name`. Only mentions a name defined elsewhere → `callee`. Needs the
owner to be pointed at → `member`.

Touching this function swaps the probe version (`ast.rs` is inside `build.rs`'s
semantic closure), so every ast-map anchor reports that the instrument was swapped
— which is correct, the output really did change. `Vocabulary.at` is not in the
closure, so adding a key to the vocabulary does not by itself swap the version.

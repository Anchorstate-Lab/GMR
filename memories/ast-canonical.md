---
about:
  - packs/coding/extract/src/ast.rs#carries
  - packs/coding/extract/src/ast.rs#spell
  - packs/coding/extract/src/ast.rs#canonical
watch: [sig, logic]
---

# The reading is taken from the tree, because taking it from the text reads the formatter too

`shape` and `body` used to be `squeeze(src[node.byte_range()])` — the rendered
source with its whitespace flattened. That makes the reading inherit every
decision the formatter made about that text, and a formatter's decisions are not
a promise to anyone. Running `cargo fmt` was enough to move an anchor: rustfmt
wraps a parameter list, adds the trailing comma that goes with wrapping, and the
byte span changes though nothing a compiler sees did. `squeeze` cannot catch that
— it normalises how *much* whitespace there is, while the comma is a real token
and the newline turned into a space beside punctuation.

So the identity comes from the tree now. What counts as content is decided by
three signals, and all three are the grammar's, not ours:

- `is_named` — `mut` is a `mutable_specifier`, so it survives;
- filling a named field — `+` is anonymous but sits in `operator`, so it survives;
- `is_extra` — the grammar itself marks comments extra, so they do not.

A `,` or a `(` is none of those. Nothing here lists a language's punctuation,
which is why the same walk serves Rust, TypeScript, Python and Go, and why a
grammar added later needs no entry.

**A node whose children all fall away keeps its own text.** Without that clause
`predefined_type` — whose entire content in TypeScript is one bare anonymous word
— would collapse to its kind, and every `number`, `string` and `boolean` would
read as the same type. The paired table has that case in it because the first
version of this walk really did merge them.

## What stops this from filtering away something real

The two tables in `tests`. One says what a formatter may do and must not move the
reading; the other says what must still move it — parameters reordered, renamed,
retyped, a `&` added, a `mut` added, an operator flipped, whitespace changed
*inside a string literal*. That last one is the difference between reading the
tree and running a regex over the text: a literal is one token, so its insides
are never touched.

The second table is the load-bearing one. A normalisation is only ever wrong in
one direction — by merging two things a compiler would tell apart — so that is
the list to grow when a new language or a new construct arrives.

## When this changes, ask

Is a filter being added that is not one of the three grammar signals? Then it is
this repository deciding what some language means, which is the line this walk
exists on the safe side of. Add the pair to the must-differ table first and watch
it fail before writing the filter.

`surface` still reads attributes through `squeeze` and drifts when a `#[derive]`
list is rewrapped. It was left alone deliberately, not overlooked.

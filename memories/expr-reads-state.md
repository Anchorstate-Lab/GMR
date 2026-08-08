---
about:
  - crates/gmr-expr/src/ast.rs#reads_state
  - crates/gmr-expr/src/ast.rs#a_string_that_looks_like_a_path_is_not_a_read
watch: [sig, logic]
---

# A read is a shape in the tree, not a substring of the rendered text

`reads_state` walks the parsed `Node` tree and asks whether any `Path` in it
has `root == Root::State`. It does not look at the rendered source string,
because a string literal can *contain* text that looks like a path —
`{ note: "state.x" }` renders with `state.x` inside it, but that `Node` is a
`Lit`, never a `Path`. Only the tree tells the two apart; a substring match
on rendered text cannot, since by the time you have rendered text you have
already lost which occurrences were literals.

The same property holds for `Root::Obs` paths, which is what the sibling
test `a_string_that_looks_like_a_path_is_not_a_read` exercises on
`reads_obs` — a string that merely *looks* like `obs.x` in a quoted literal
must not count as a read of `obs.x`.

## When this changes, ask

Does the new check inspect `Node` variants, or does it inspect `render()`
output / raw source text? Anything that answers this question from text
instead of structure reopens the literal-vs-path confusion this guards
against.

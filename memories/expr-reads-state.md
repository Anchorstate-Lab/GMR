---
about:
  - crates/gmr-expr/src/ast.rs#reads_state
  - crates/gmr-expr/src/ast.rs#a_string_that_looks_like_a_path_is_not_a_read
  - crates/gmr-expr/src/ast.rs#a_quantifier_body_does_not_read_the_state_the_accumulator_warning_is_about
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

## A quantifier body reads a different `state`, so it does not count here

`all(anchors, state.v.sig)` reads `state`, and `reads_state` answers **false**.
That is not an omission. The one caller is `accumulator_warning`, which asks
whether a rule folds its own previous answer into its next one — the thing that
over-counts when an observation repeats without a lease. Inside a quantifier,
`state` is one of the anchors the invariant is about, and it was never this
anchor's accumulator. Counting it would fire the warning on every invariant ever
written, which is how a warning stops being read.

`state.n and all(anchors, state.v.sig)` is still true: the outer read is real.

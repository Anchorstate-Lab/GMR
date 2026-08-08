---
about:
  - crates/gmr-expr/src/ast.rs#reads_obs
  - crates/gmr-expr/src/ast.rs#collect_obs
  - crates/gmr-expr/src/ast.rs#changed_is_a_read_of_obs
  - crates/gmr-expr/src/ast.rs#an_index_ends_the_path
watch: [sig, logic]
---

# `changed(x)` is a read of `obs.x`, and an index ends the path it names

`reads_obs` returns dotted field names rather than a raw `Path`, because
that dotted name is exactly what `changed()` faults on: `changed("matches")`
has to be reported as reading `obs.matches`, otherwise the rule engine could
not tell which fact a `changed()` guard depends on. That is why
`collect_obs` folds `Node::Changed(name)` straight into the same output set
as `Node::Path` under `Root::Obs` — from the rule engine's point of view they
are the same dependency.

An index step (`obs.matches[0]`) ends the dotted path at the field before
it: an index does not name a further field to check for presence, it
selects an element of one that was already checked. `collect_obs` stops
collecting field steps at the first `Step::Index`, which is what
`an_index_ends_the_path` pins down (`obs.facts.files[0].name` reads as
`facts.files`, not `facts.files.name`).

## When this changes, ask

Does a new `Node` variant represent something that reads a fact from
`obs`? If so it needs an arm in `collect_obs`, or `reads_obs` silently
under-reports what a rule depends on — and a rule's guard can then go stale
without anyone re-checking it.

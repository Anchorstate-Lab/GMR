---
about:
  - domains/coding/extract/src/lang.rs#Table
  - domains/coding/extract/src/ast.rs#members
  - domains/coding/extract/src/ast.rs#attributes
watch: [sig, logic]
---

# A signature is what this definition promises its callers, not which fields tree-sitter handed over

`shape_fields` fetches through `child_by_field_name`, so it can only reach the
**named fields** of the grammar. And half of the things that mean "every caller
has to change" are not in fields at all:

| | where it lives | what fetches it |
|---|---|---|
| parameters · return type · type parameters | fields | `shape_fields` |
| `async` `unsafe` `const` · where clauses | unnamed child nodes | `shape_kinds` |
| `#[derive]` `#[deprecated]` · TS/Python decorators | **preceding siblings** | `Attrs::Before` |
| struct fields · enum variants · trait method signatures | the body list of a child | `members` |

These four are not four special cases, they are four places the same sentence
lands in the syntax tree. The criterion is: **take it away — can the caller still
compile unchanged?** If yes, it is not part of the signature.

## Three concrete judgments

**One `Attrs::Before` variant is enough.** All three languages were measured to use
preceding siblings: Rust's `attribute_item`, TS's `decorator` (which sits before
`class_declaration` inside `export_statement`), Python's `decorator` (inside
`decorated_definition`). It has to use `prev_named_sibling` and not
`prev_sibling` — TS's `export` is an anonymous token, and hitting it in between
breaks the loop.

**`NOISE` is a blacklist, not a whitelist.** A whitelist makes a newly appearing
attribute **silent**; a blacklist makes it speak up, and then you decide whether
to shut it up. "The system allows no silent failure path" permits only the
latter. What the nine on the list (`allow` `warn` `deny` `expect` `inline` `cold`
`doc` `rustfmt` `clippy`) have in common is that removing them means no caller
changes a character. `serde` is not among them — it changes the wire format.

**A type's shape is its members, and the body is left holding only their
implementations.** A struct has no `parameters` and no `return_type`, so its shape
used to be permanently empty: ten of this repository's twenty-five contract
anchors were running with a dead axis, and "a field was added" reported
`logic-changed` — saying "go re-read the implementation" when it meant "go look at
every construction site". Split apart, a struct has no implementation at all and
adding a field is a signature change; a trait's default method bodies still drive
`logic` independently.

## `at` has two layers, and only one of them is identity

```
ITEMS       matchable  — decides which candidate a coordinate selects.
                         In the semantic closure; changing it swaps the probe version
at \ ITEMS  observable — form · surface · after. Bounded, printable, does not match
facts       measured   — facts about the already-matched object; hashable, may be unbounded
```

That middle layer used to be implicit: I put things in `at` on the basis of "small
enough" and never wrote the criterion down. The condition for going in is
**bounded · printable · not identity**. That last one matters most — the moment a
key both decides which candidate is selected and is treated as an observable
direction, it can never move again, because the selected candidates are by
definition equal on it. That is how the `file` axis died; see [[shapes-Dim]].

`every_matchable_key_is_one_the_probe_declares` pins `ITEMS ⊆ at`: declaring a
matchable key that is never emitted makes every position written with it fail to
match, silently.

## Naming a `use_declaration`

`gmr`'s public surface is entirely `pub use`, and `use_declaration` has no `name`
field — so those entries once had **no identity at all**, five candidates crammed
into five empty strings in the roster.

The fix was not to invent a fallback id (a byte offset changes on any edit above
it, which is the same hair-trigger relocated), it was to give them real names: the
`argument` field, that is, the import path itself. `Table.names` is a chain of
identity fields tried in order, the same shape as `shape_fields`.

**A fallback id that is unstable under unrelated edits is more honest thrown away.**
But neither of those is the answer — the answer is closing the gap in the
representation layer.

## When this changes, ask

Adding something to `shape_fields` or `shape_kinds` → apply the criterion: take it
away, can the caller still compile? No → it belongs to the signature. Yes → it is
noise, and adding it teaches people to ignore this axis.

Adding an entry to `NOISE` → state the reason "removing this attribute means no
caller has to move". If you cannot say it, do not add it.

`members` changes how it expands (recursing into nested types, say) → the `sig` of
every type anchor moves, and it is **a change of criteria, not a change of fact**:
go through `rebase --all`, do not accept it as drift.

All three of these functions are inside `build.rs`'s semantic closure — touch one
and the probe version swaps and the whole repository needs a `rebase`. So if you
are going to change them, change them all at once — see [[probe-Derivation]].

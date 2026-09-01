---
about:
  - batteries/survey/src/recipe.rs#Recipe
  - batteries/survey/src/recipe.rs#look
  - batteries/survey/src/recipe.rs#Merge
  - batteries/survey/src/recipe.rs#the_eligible_predicate_decides_what_the_corpus_even_contains
  - packs/coding/extract/src/ast.rs#RECIPE
  - packs/coding/extract/src/name.rs#RECIPE
watch: [sig, logic]
---

# An extractor is nine declared things, and no body of its own

Everything that distinguishes one extractor from another is a field on
`Recipe`. `look` is the single body all of them run, so a probe function is one
line.

```
name         the probe's name          half of the index address
version      the earned hash           the other half
items        which position fields form the want, in priority order
narrows_on   which want keys may narrow the index query
identity     which want keys make a candidate eligible at all
eligible     which files this probe can read
collect      the per-file parse, emitting fragments
merge        Concat | Fold             whether fragments are already candidates
barren       what an empty corpus means for this probe
```

An extractor that needs behaviour `look` does not have is asking for a tenth
field. It is not asking for a body of its own: a second body is a second place
for the walk, the barren check, the narrowing decision and the report to drift
apart, and the four extractors here differ in none of those.

## `collect` takes bytes, and each extractor decodes for itself

`addr-map` reports `bytes.len()` and fingerprints content, so the parameter
cannot be `&str`. The three that want text call `str::from_utf8(bytes).ok()`
and skip the file when it fails, which is what makes "not valid text" a file
that contributes nothing rather than a reading that refuses. Decoding lossily
instead would let previously-invisible files produce candidates — an output
change, not a decoding detail.

## `eligible` decides what the corpus contains, not what the query sees

A file the predicate rules out is never read, so it cannot contribute a
fragment under any coordinate.
`the_eligible_predicate_decides_what_the_corpus_even_contains` pins that: the
same tree seen through `anything` and through `rust_only` yields a different
candidate count for one identical query.

That is why the predicate belongs to the extractor and rides inside its earned
version. It is an input to every answer the probe gives, so a change to it has
to change the index address ([[survey-index-shape]]) — otherwise a generation
built when the predicate was narrower keeps serving answers the current
predicate would not produce.

## Order follows from `merge`; it is not a member

`Concat` yields walk order, which the index reproduces by sorting on
`(sort, ord)` ([[survey-index-shape]]). A `Fold` yields whatever its own key
gives — `name-map`'s is `(name, scope)`, which is not a path at all. Declaring
an `order` member would mean inventing a question with two answers already, one
of which is "not applicable".

## `identity` gates eligibility; `items` order sets priority

They are separate because they answer separate questions. `items` order is the
priority `report` ranks by ([[survey-report]]). `identity` names the keys that
make a candidate worth considering: when the want touches any of them, a
candidate must hit at least one to be eligible; when it touches none, a
candidate must hit every wanted pair.

`ast-map` declares `name`/`callee`/`member`/`shape` and not `file`, so a
coordinate naming a file and a symbol cannot return every node in that file
merely because they all match on `file`.

## When this changes, ask

Does a field arrive that some extractor cannot answer? Then it is two fields,
or it belongs to `merge` — `Recipe` is what every extractor declares, and an
`Option` here means the shape is wrong.

Does an extractor grow a body beside `look`? Whatever it needed is a field, and
the second body is where the walk and the report start disagreeing.

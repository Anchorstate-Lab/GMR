---
about:
  - batteries/survey/src/recipe.rs#narrowable
  - batteries/survey/src/narrow.rs#touches
  - batteries/survey/src/narrow.rs#narrow
watch: [sig, logic]
---

# Only a key the merge carries through verbatim may narrow the query

`narrowable` keeps the wanted pairs whose key is in `narrows_on` and drops the
rest. `look` asks the index for the union of rows touching what survives, and
reads the whole generation when nothing does. That is what lets a question about
one symbol be answered without materialising the repository.

The rule the field has to obey is narrower than "a key this probe declares":

> `narrows_on` may name a key only if the value a candidate carries under it is
> the value some fragment already carried under it, unchanged.

For a `Concat` recipe that is every item, because the candidate *is* the
fragment. For a `Fold` it is only what the fold copies across untouched —
`name-map` folds fragments keyed `(name, file)` into candidates keyed
`(name, scope)`, so `name` may narrow and `scope` may not.

## A key the fold derives cannot narrow the fold

`scope` is computed from `file` while folding, and appears in no fragment's
coordinate at all. Narrow on it and the index is asked for rows matching a key
no row has: it returns nothing, the fold runs over nothing, and the answer is
`found: false` for a name that is genuinely there. The full read answers
`found: true` for the same coordinate.

That failure is silent and shape-specific — it needs a coordinate naming only
derived keys, so it hides from any test whose position happens to include a
carried one. The safety is structural instead: a fold declares the keys it
carries, and everything else falls back to the whole generation. The fallback
still skips the parse, which is the expensive half, so it costs a wider read
rather than a rebuilt corpus.

## The predicate is union, not intersection

`touches` selects a coordinate that matches **at least one** wanted pair, and
the index's `union` answers the same question. Anything matching none of them
cannot appear in the report under any `nth` ([[survey-narrow]]), so dropping it
before the fold changes no answer — while intersecting would drop candidates
that tie on the winning vector and change which one `nth` selects.

## When this changes, ask

Does a fold start deriving a key it also declares in `narrows_on`? That is the
silent `found: false`, and it will look like the coordinate is simply wrong.

Does anything narrow on the want rather than on `narrowable(want)`? The two are
the same only for `Concat` recipes, which is exactly the case that would let it
pass every test until a fold is pointed at a derived key.

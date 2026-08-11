---
about:
  - batteries/survey/src/recipe.rs#Recipe
  - domains/coding/extract/src/ast.rs#RECIPE
  - domains/coding/extract/src/name.rs#RECIPE
  - batteries/survey/src/recipe.rs#look
  - batteries/survey/src/recipe.rs#Merge
  - batteries/survey/src/cache.rs#gather
  - batteries/survey/src/cache.rs#scan
  - batteries/survey/src/cache.rs#a_file_the_recipe_rules_out_leaves_no_trace_in_the_cache
watch: [sig, logic]
---

# An extractor is six things, and it was read out of the four that exist

The four `probe` bodies were word-for-word identical. Reading them side by side,
everything that differed is a field:

```
name       "ast-map"                       the scope the cache is keyed by
version    env!("GMR_EXTRACTOR_AST")       the earned hash it reports as `extractor`
items      ITEMS                           which position fields form the want
eligible   lang::for_path(rel).is_some()   which files this probe can read at all
collect    fn(rel, bytes, out)             the per-file parse
merge      Concat | Fold(rolled)           whether candidates are the fragments
barren     "contains no parseable nodes"   what an empty corpus means for this probe
```

That is `Recipe`, and `look` is the body. It was derived, not designed: three
things the plan assumed turned out not to be true of the code.

## `parse(rel, src: &str)` cannot express `addr-map`

The plan had extractors take a `&str`. `addr-map` needs **bytes** — it reports
`bytes.len()` and fingerprints the content — so `collect` takes `&[u8]` and each
extractor decodes for itself. `ast`, `name` and `prose` use
`str::from_utf8(bytes).ok()`, which is what `read_to_string` already was: it
fails on invalid UTF-8 and the file is skipped. Swapping in `from_utf8_lossy`
would have made previously-invisible files produce candidates, which is an
output change wearing the clothes of a refactor.

## `order` is not a member; it follows from `merge`

The plan listed `order()` alongside the rest. It has no counterpart in the code:
`Concat` yields walk order, and a `Fold` yields whatever its own key gives —
`name-map`'s is `(name, scope)`, which is not a path at all. Declaring an order
separately would have been inventing a member and then finding two
implementations for it. See [[survey-index-shape]] for where a sort key does
become necessary, and note it is derived from `rel` there rather than declared
here.

## Splitting `Fragment` from `Candidate` does not belong to this step

[[survey-fragment-debt]] sets the trigger precisely: **before `narrow` gets its
first caller**. That is the query rewrite, not this. Bringing the split forward
would have forced the cache's `Entry` to change type, which drags the four
extractors in and turns one commit into a coupled pair for no benefit anyone
could name yet.

## Two costs the old signature was hiding

**Every file was read twice on a cache miss.** `scan` reads the bytes to hash
them for freshness, then the old `collect` took a `&Path` and read the same file
again. Handing the bytes on removes the second read;
`the_bytes_the_recipe_is_handed_are_the_ones_the_cache_hashed` pins it by
fingerprinting what `collect` received and comparing it to the entry the cache
keyed.

**Every file was read, hashed and cached — including ones no extractor could
use.** `prose-map` handles `.md` and this repository has 305 files; `ast-map`
handles source and 129 of them are. The rest were read, hashed, handed to
`collect`, and stored with an empty candidate list. `eligible` now runs before
the read, so they are not touched at all.

`eligible` belongs to the extractor and therefore to its earned version, which
is not a stylistic choice: [[survey-cache-scope]] warns that `Vocabulary.handles`
lives in `lib.rs`, outside the closure, and that using it as a scan filter would
repeat that incident. A predicate that decides which files reach `collect`
decides what the answer can contain, so it is exactly the kind of input Rule 5
means by *earned*.

The budget checkpoint stays **ahead** of the predicate. A tree of a hundred
thousand ineligible files still has to be interruptible, and a cheap skip that
cannot be cancelled is a slow scan with no way out.

## The fold still has to be remembered

`name-map` is the only `Fold`, and [[name-map-cache]] measured what memoising it
is worth: 467ms to 7.4ms. `Merge::Fold` therefore does not run in `look`; it runs
through `cache::folded`, which is the same `Flight` slot `visit_folded` used.
`Merge::Concat` hands the gathered `Arc` straight back — a fold memo for the
identity would be a full copy of the corpus on every question.

## The translation was proved, not hoped

Landing this earns all four extractors a new version, which is only honest if
the answers did not move. 1149 readings — every `path#symbol` coordinate this
repository's own memories declare, each asked of the probe that owns it and of
the ones that do not, plus roster shapes and `nth` sweeps — run against a frozen
copy of the tree under both codes: 941 `found`, 208 `found:false`, and **zero
differences apart from the `extractor` field itself**.

The barren branch has no reading in that set, which is worth saying rather than
leaving to be discovered: it is covered by unit tests in this file, not by the
sweep.

## When this changes, ask

Does `collect` still receive the bytes the scan already read, or has something
gone back to taking a path? And is `eligible` still inside the closure — a
predicate that filters the corpus from outside it is a version that does not
change when the answer does.

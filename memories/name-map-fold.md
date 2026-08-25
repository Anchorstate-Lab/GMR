---
about:
  - domains/coding/extract/src/name.rs#collect
  - domains/coding/extract/src/name.rs#merge
  - domains/coding/extract/src/name.rs#rolled
  - domains/coding/extract/src/name.rs#scopes_of
  - domains/coding/extract/src/name.rs#probe
  - domains/coding/extract/src/name.rs#scopes_are_every_prefix_of_the_path
  - domains/coding/extract/src/name.rs#a_path_whose_scopes_are_not_its_prefixes_is_refused_not_dropped
  - domains/coding/extract/src/name.rs#a_fresh_corpus_and_a_reused_one_agree_and_repeating_the_query_does_not_move_it
watch: [sig, logic]
---

# A cross-file total is a per-file fragment and a fold, because that is the only part worth storing

`name-map` answers about something no single file holds: `occurrences` sums over
the repository, `files` unions over it, `first` depends on walk order. The index
still holds only per-file rows, because the aggregate comes apart cleanly.

```
collect(file)    one fragment per identifier: coord {name, file}, facts {count, line}
merge(walk order)  for each fragment, for each scope prefix of its path:
                   count += count · files.insert(rel) · first.get_or_insert(..)
rolled           the map, keyed (name, scope), as candidates
```

Only `collect` depends on a file's bytes, so only its output is worth keeping:
it is what a file's content hash actually addresses. The fold is a pure function
of the fragments and is cheap to redo, so it runs per query and nothing persists
it. Storing folded totals instead would key them on the whole corpus, which
means one file changing invalidates all of them.

## `first` is well-defined only because the index preserves walk order

`get_or_insert` keeps the earliest fragment it sees, so `first` means "first in
walk order" and nothing else. The index hands rows back ordered by
`(sort, ord)`, which reproduces exactly that order ([[survey-index-shape]]) —
a backend that returned rows in path-byte order would rename which file
`first` points at without a line of this file changing.

## Scopes are every prefix, and a path that has none is refused

`scopes_of` returns `""` and each `/`-separated prefix, so one identifier in
`a/b/c.rs` contributes to `""`, `a`, `a/b` and `a/b/c.rs`. That is what makes
`scope` a containment question rather than a directory-equality one.

An empty path component means the caller did not build this path from directory
entries, and the prefixes would not be prefixes. `scopes_of` refuses the whole
reading rather than dropping the file: a silently skipped file lowers
`occurrences` for every scope above it, and nothing downstream can tell a real
zero from a dropped one.

## `scope` is folded, so it cannot narrow the query

`name` is carried from the fragment unchanged and may narrow; `scope` is derived
here and may not. `narrows_on` declares exactly that — see [[survey-narrows-on]]
for what goes wrong when a derived key is used to narrow.

## When this changes, ask

Does anything persist the folded totals? Their validity is corpus-wide while
the index's unit of invalidation is one file, so they go stale on any edit
anywhere and nothing detects it.

Does `merge` start depending on something other than the fragments it is
handed — the tree, the clock, a path on disk? It is run per query over rows read
back from the index, so anything it reads that is not in a fragment is a second
source of truth for an answer the index already claims to hold.

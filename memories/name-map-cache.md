---
about:
  - domains/coding/extract/src/name.rs#collect
  - domains/coding/extract/src/name.rs#merge
  - domains/coding/extract/src/name.rs#scopes_of
  - domains/coding/extract/src/name.rs#probe
  - batteries/survey/src/cache.rs#folded
  - batteries/survey/src/cache.rs#an_aggregate_is_folded_once_per_scan_not_once_per_question
  - domains/coding/extract/src/name.rs#caching_changes_nothing_about_the_answer
watch: [sig, logic]
---

# A cross-file total is a per-file thing, a fold, and a memo for the fold

`name-map` was the only extractor that took a `Cache` and ignored it — the
parameter was literally `_cache`. Every query re-walked, re-read and
re-tokenised the whole repository: 364ms per call here, 467ms on an 800-file
fixture, and a second identical call cost exactly the same as the first.
`ast-map`, which does strictly more work, answered a warm query in 2ms.

It was skipped because its candidates are a cross-file aggregate — `occurrences`
sums over the repository, `files` unions over it, `first` depends on walk order
— while the cache caches **per file**, keyed by content hash. Wrong shape,
apparently.

It is not, because the aggregate is a monoid and comes apart cleanly:

```
collect(file)   one candidate per identifier: its count here, its first line here
merge(walk order)   for each fragment, for each scope prefix of its path:
                    count += count · files.insert(rel) · first.get_or_insert(..)
```

## Two halves, two different costs, and only fixing one is not fixing it

Caching the first half alone took 467ms to 166ms — barely a third off, and the
number said why. `gather` already memoises the scan per scope, so a
second call in the same process skips the disk entirely; that 166ms was
**merge, and nothing but merge**. Borrowing the scopes instead of allocating
them (they are all prefixes of `rel`, so they can be `&str`) took it to 143ms.
The remainder is not overhead, it is the aggregation itself: O(identifiers ×
depth) per question, and no amount of tightening removes work that is being
redone.

The fold is a pure function of the fragments, and the fragments are already
settled for the scope. So `folded` memoises it in the same `Flight` that
holds the scan. 467ms → **7.4ms**. On this repository 364ms → 7.9ms; on a
3200-file tree six directories deep, 2.5s cold and 65ms warm.

`Flight` is per process and per scope, which is the honest scope for this: the
fragments on disk survive restarts, the aggregate does not. A run with many
`name-map` anchors pays the fold once instead of once per anchor, which is the
case that was hurting.

## Why `first` needs no `min` and no sort

`gather` returns fragments in walk order, on the cached path as well as
the cold one, because both extend the same vector inside the same `visit`. So
folding with `get_or_insert` picks the first *file* in walk order carrying that
file's own first line — exactly what the nested loop produced. Writing it as
`min((rel, line))` would have required the comparison to agree with `walk`'s
component-wise `PathBuf` order, which is **not** byte order in a tree holding
both `foo.rs` and `foo/` — see [[survey-walk]]. Keeping the fold order sidesteps
the question entirely.

## `scopes_of` returns a `Result` because borrowing has a precondition

Scopes are slices of the path, which only works while every scope is a prefix
of it. `a//b.rs` breaks that: its scopes are `a` and `a/b.rs`, and `a/b.rs` is
not a substring. `visit` builds `rel` from directory entries so it cannot happen
— but the way to encode "cannot happen" is a refusal that reaches the caller,
not a `continue` that drops the file out of every count in silence.

## Proving a version bump is a translation

`name.rs` is in `name-map`'s closure, so this earns a new version and every
`name-map` anchor rebases. That is only honest if the answers did not move, so
they were compared: 896 positions built from this repository's own 5898
identifiers and 37 scopes, run under both the old and the new code against two
frozen corpora — byte-identical on every one.

The corpus has to be frozen. The first attempt compared against the working
tree and reported `occurrences` for the name `scope` moving 52 → 58. Not a bug:
`name.rs` is part of the corpus it measures, and editing it changed the counts.
Any future comparison of an extractor against its own repository has to take a
snapshot first, or it measures the edit.

## When this changes, ask

Does `collect` still depend on nothing but this one file's bytes and its path?
The moment it reads a sibling, the per-file key is a lie and the failure is
silent — see [[survey-cache-scope]]. Does `merge` still consume fragments in the
order it receives them? A sort, a `HashMap`, or a parallel walk in between moves
`first` for every name whose first two files differ in walk order versus byte
order, and no flat fixture would notice. And is anything but a pure function of
the fragments being handed to `folded`? A fold that reads the clock, the
filesystem, or the position would be memoised along with its answer, and the
second question in the process would get the first question's reply.

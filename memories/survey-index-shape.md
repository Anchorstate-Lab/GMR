---
about:
  - batteries/survey/src/index.rs#Generation
  - batteries/survey/src/index.rs#under
  - batteries/survey/src/index.rs#Indexed
  - batteries/survey/src/index.rs#Index
  - batteries/survey/src/index.rs#Snapshot
  - batteries/survey/src/index.rs#a_generation_is_the_probe_and_the_version_and_not_the_root
  - batteries/survey/src/index.rs#the_two_halves_of_a_generation_cannot_be_slid_into_each_other
  - batteries/survey/src/index.rs#a_root_selects_what_is_under_it_and_not_what_merely_starts_with_it
  - batteries/survey/src/walk.rs#sort_key
  - batteries/survey/src/walk.rs#the_sort_key_reproduces_the_order_the_walk_hands_files_over_in
  - batteries/survey/src/walk.rs#sorting_the_same_paths_by_their_bytes_would_not_have_agreed
  - batteries/survey/src/testkit.rs#Remembered
watch: [sig, logic]
---

# One index per probe and version; the root is a predicate, and the writer owns the order

## The address is everything that decides the answer, and nothing else

`Generation::of(probe, version)` — those two, hashed with a NUL between them so
a longer probe name cannot eat a shorter version and leave two probes sharing
one index.

Both halves have to be there. The version is the extractor's earned hash, which
covers its parse, its eligibility predicate and its declared vocabulary
([[survey-recipe]]), so an extractor that starts seeing more opens a new
generation instead of reading a built one that could not see it. An address
missing an input serves answers the current code would not produce, and the
files are unchanged, so nothing looks wrong.

**The root is not one of those inputs.** A file's fragments are the same facts
whoever is asking; a narrower root asks a different question about them rather
than creating new ones. So the root stays out of the address and becomes a
predicate on the query, and opening an anchor that narrows costs zero index
bytes ([[lib-narrow_of]]).

`under(rel, root)` decides it, and what it must get right is that a root selects
what is *beneath* it, not what shares its opening characters:
`crates/gmr-core` must not draw in `crates/gmr-core-extra`. A `LIKE 'root%'` in
any backend gets that wrong.

## The writer supplies the sort key; the index only sorts by it

`Indexed` carries a `sort` string and rows come back ordered by `(sort, ord)`.
The index never derives that key from the path, because path order is not string
order: `walk` sorts `PathBuf`s, which compare component by component, while SQL
`ORDER BY rel` compares bytes. They disagree wherever a file and a directory
share a stem — `b.rs` against `b/x.rs`, `mod.rs` against `mod/a.rs` — because
`'.'` is 0x2E and `'/'` is 0x2F, while a component comparison finds `b` a prefix
of `b.rs` and therefore smaller. Rust without `mod.rs`, TypeScript and Python
all lay repositories out that way.

This is not cosmetic. `report` reads `nth` as an index into the tied candidates,
so a backend that reorders here renames which object an anchor is about while
nobody has touched the code, and `name-map`'s `first` names a different file
([[name-map-fold]]). The conformance suite pins the exact pair: `b/x.rs` must
come back before `b.rs`.

`sort_key` is that definition, once, in `walk.rs` — a fact about a path, beside
the walk that produces one. The writer calls it; a path-shaped writer is only
one kind, and `name-map` orders by `(name, scope)` and has no use for it.

## Two backends, one conformance suite

`SqliteIndex` and `testkit::Remembered` answer the same suite, so the shape is
checked against something faster than SQLite and the fast one cannot quietly
mean something else. A property that only one of them satisfies is not a
property of `Index`.

`Built::whole` and `Snapshot::whole` are `sealed_at.is_some()`: a generation is
complete when a walk finished and said so, never because it happens to hold
rows. Writing opens a generation; sealing one nothing opened would record a
completeness nobody earned, and `Index::unopened` refuses it.

## When this changes, ask

Does something reach the answer that is not in the address and not a file's
content hash? It has to fold into `Generation`, or a built index outlives the
thing that decided what it holds.

Does a backend order rows by anything but `(sort, ord)`? That silently renames
what an anchor watches, and every test that checks *which* candidates come back
still passes.

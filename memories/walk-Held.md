---
about: batteries/survey/src/walk.rs#Held
---

# Held/Stamp moved here for their second producer; the SQLite backend needed a place to keep them that neither storage layer owned

`Stamp` (mtime_ns + size) and `Held` (hash + stamp) used to live in
`corpus.rs`, defined right beside the in-memory `rescan()` that was their
only producer. Giving `SqliteIndex` a second producer — `Index::known()`
now returns `BTreeMap<String, Held>`, not a bare hash — meant `Held` needed
a home two different layers could reach without depending on each other.
`corpus.rs` (the sync `Corpus`/`rescan` algorithm layer) and `index.rs`/
`sqlite.rs` (the async storage layer) were, and still are, peers with no
dependency between them; both already imported `hash()` from `walk.rs`.
Moving `Held`/`Stamp` there instead of into either peer avoids inventing a
new dependency edge — the same shape [[survey-index-shape]] used when
`sort_key` needed to move for its own first real producer.

`file` in the SQLite schema gained `mtime_ns`/`size` columns,
`SCHEMA_VERSION` 1 -> 2. Free per [[survey-index-sqlite]]'s own rule: the
index holds nothing that isn't re-derivable from the tree, so a version
bump is razed and rebuilt, never migrated — these two columns are no
exception.

## `Index::restamp` exists so a touched-but-unchanged file doesn't pay a full rewrite

`corpus::rescan` already splits what it finds into `fresh` / `restamped` /
`gone`. A `restamped` file's content hash matched what was known — its rows
don't need touching — but its stamp changed and has to be persisted, or the
next process's freshness check will see a stale stamp, mismatch, and
re-hash the file all over again even though nothing in it actually moved
(a `git checkout`, a build tool, anything that touches mtime without
touching bytes). Without a dedicated `restamp`, the only way to persist a
new stamp would have been calling `write()` — which deletes and reinserts
every candidate/posting row for that file — for a change that is, by
definition, exactly the case where nothing in those rows moved.
`restamp` is one batched `UPDATE ... WHERE generation = ? AND rel = ?`
transaction, matching the batching discipline [[survey-cache-write]]
measured the old `Cache` needed and didn't have at first: per-file writes
on that path were the thing that cost forty times what caching saved.

## When this changes, ask

Does a new `Index` implementor forget `restamp`, silently falling back to
calling `write` for a restamp (or not persisting the new stamp at all)?
Either one is a correctness-preserving but perf-losing regression that no
test catches by watching answers — only by watching how many files get
re-hashed on a second, otherwise-idle run.

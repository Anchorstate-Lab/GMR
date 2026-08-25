---
about:
  - batteries/survey/src/walk.rs#Held
  - batteries/survey/src/walk.rs#Stamp
  - batteries/survey/src/corpus.rs#rescan
watch: [sig, logic]
---

# What a file on disk currently is, in the one place both layers already depend on

`Stamp` is `mtime_ns` + `size`; `Held` is a content hash plus that stamp. Both
are facts about a file on disk, so they sit in `walk.rs` beside `visit`,
`hash` and `sort_key`.

They need that home because they have two producers in layers that are peers:
`corpus::rescan` computes them while walking, and `Index::known` hands them back
out of storage. `corpus.rs` and `index.rs`/`sqlite.rs` have no dependency
between them and should not gain one; both already depend downward on `walk.rs`,
so putting `Held` there adds no edge.

## The freshness ladder is three rungs, and each one skips more work

`rescan` sorts every eligible file into `fresh` / `restamped` / `gone`:

```
stamp matches what is known   skip entirely — no read, no hash, no parse
hash matches                  restamped: the bytes are the same, the stamp is not
otherwise                     fresh: read, hash, and run collect
```

The middle rung is the one that needs `Index::restamp`. A `git checkout`, a
build tool, anything that touches mtime without touching bytes lands there, and
its rows do not need changing — but the new stamp still has to be persisted, or
the next run reads a stale stamp, mismatches, and re-hashes the file again
forever. `restamp` is one batched `UPDATE`, where `write` would delete and
reinsert every row for that file: a full rewrite for the one case defined by
nothing in those rows having moved.

`Stamp::of` returns `Option`, and a file whose metadata cannot be read simply
has none — it then falls through to the hash rung rather than being dropped,
which is why the stamp is an optimisation and never an answer.

## When this changes, ask

Does a new `Index` implementor answer `restamp` by calling `write`, or by doing
nothing? Both keep every answer correct, so no test that checks answers fails —
the cost only shows as files being re-hashed on a second, otherwise idle run.

Does anything start treating a matching stamp as proof of matching content? It
is a cheap guess that lets the hash be skipped; the hash is what decides.

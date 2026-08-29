---
about:
  - domains/node/bench/latency.mjs#timed
  - batteries/provider/src/git.rs#fetch
watch: [sig, logic]
---

# Grounding one sentence costs a fold and a subprocess, and the subprocess is all of it

Measured through the addon, which is what a caller actually pays. One anchor,
one sentence, one sqlite store, on a warm laptop:

```
ground, no store wired (journal + fold only)   p50  0.18 ms
ground, served from the record                 p50  6.37 ms
ground, forced to look again                   p50  6.08 ms
since(0), every anchor's record fetched        p50  6.31 ms
since(0, status), no record fetched            p50  0.19 ms
```

The numbers themselves are a laptop's and will not reproduce. **The shape will**,
and the shape is the finding: retrieving the memory record is roughly **thirty
times** everything else put together, and the two rows that skip it land in the
same fifth of a millisecond as each other.

Everything GMR was careful about is in that 0.18 ms — the journal read, the
incremental fold, both axes of the [[runtime-warrant]], the whole of
[[runtime-ground]]'s five phases. Forcing an observation adds almost nothing
here because the probe is a small local file; a network probe would move that
row and not the others.

## Why the fetch costs what it does

`git::fetch` runs `git` as a subprocess, once per record. That is a process
spawn, an exec, and a repository open, for a file the filesystem could hand over
in microseconds — and it is bought deliberately: a git object is addressed by a
content hash the repository computes, which is what makes a memory's version
something [[runtime-grounding]] can compare rather than something GMR asserts.

The number to keep in view is that **the choice of memory store, not GMR, sets
the latency a caller sees.** A store reached over a network will dominate more
than this one does.

## `since` has the same split, and a status filter is the switch

`changed_since(cursor, None)` builds `raised`, which needs each anchor's record
and therefore one fetch each. Passing a status skips that entirely. A caller
polling for edges on a large corpus is choosing between those two rows every
time they call it, and nothing in the signature says so.

## Re-running it

`sh domains/node/bench.sh`, optionally with `GMR_BENCH_ANCHORS` to put more
anchors on one sentence. It is not in CI: a timing that fails a build on a busy
runner teaches people to ignore the build.

## When this changes, ask

Did a store fetch stop dominating? Then either the store changed or something
started caching a record, and a cached record is a version compared against
itself.

Did the 0.18 ms row grow? That is the part GMR owns, and it is where an added
per-call read — a full journal scan, a re-fold, a second projection — would
show up first.

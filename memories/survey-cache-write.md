---
about:
  - batteries/survey/src/cache.rs#persist
  - batteries/survey/src/cache.rs#replace
  - batteries/survey/src/cache.rs#flight
  - batteries/survey/src/cache.rs#a_scan_writes_the_cache_file_once_not_once_per_file
  - batteries/survey/src/cache.rs#a_failed_scan_is_not_retried_by_the_next_caller
watch: [sig, logic]
---

# The cache is written once per scan, and a scan that failed is not run again

`put` only touches memory and sets `dirty`. The file is written exactly once,
by `persist`, after `visit` has returned. This is not a micro-optimisation.

The first version wrote inside the per-file loop: clone the whole entry map,
serialise all of it to JSON, overwrite the file — once for **every file**,
including the ones that produced no candidates at all (binaries, lockfiles,
images). That is quadratic in the size of the repository, and it was measured,
not guessed. On synthetic trees at realistic candidate density:

```
files    per-file write     once per scan
  200          2.59 s            0.18 s
  400         10.55 s            0.36 s
  800        41.46 s            0.73 s      <- 41 s against a 30 s probe budget
```

4.0x per doubling before, 2.0x after. The 800-file run serialised and wrote
roughly 9.2 GB to land a 23 MB file. Parsing was never the problem — with the
cache switched off entirely that same tree took 1.03 s. **The cache cost
forty times what it saved**, and on a large enough repository it exhausted the
probe budget on every coordinate, so `ast-map` looked like it could not answer
anything at all.

`replace` writes to a temporary and renames. Rename is atomic on the same
filesystem, so a process that dies mid-write leaves either the old file or the
new one, never half of one. That matters more than it looks: `main` now ends
with `shutdown_background()` (see [[cli-main-run]]), which detaches still-running
blocking threads and lets them die at process exit — possibly inside this
function. Without the rename, that exit path corrupts the cache, and a corrupt
cache is silently discarded on the next load, so the repository would quietly
pay a full scan forever with nothing to read that said so. That is why `load`
now separates "no file yet" (fine, start empty) from "a file that will not
parse" (reported, never swallowed).

## Why a failed scan is remembered

`flight` memoises the **`Err` as well as the `Ok`**. Anchors are observed one
after another in a single `pass`, and every one of them that names the same
probe and root asks for the same scan. The old memo was only recorded on the
success path, so the first timeout left it empty and anchors two through N each
started **another** full-tree scan on **another** blocking thread — all of them
contending on the one mutex around the entry map. One slow repository therefore
did not produce one timeout, it produced a thread per anchor, which is what
pegged a core for six minutes after the CLI had already exited.

Refusing to retry inside one process is the point: the second caller wants the
same answer to the same question, and that answer is "this failed". It surfaces
once, with the real reason, instead of once per anchor as a timeout.

The memo is only active when there is a file to persist to. `Cache::disabled()`
keeps the per-file map but no scan-level memo, so callers that rewrite a file
and immediately re-probe the same directory — which is exactly what the shape
and extractor tests do — still see the change.

## Once per scan is not once per run, and a per-scope flag does not close the gap

`persist` writes at the end of each scan, and a cold `gmr check` on this
repository scans eight times — once for the root and once for each `layer::*`
anchor's narrower root (see [[layers]]). Eight full-file writes to land one
6.3 MB file.

The obvious fix is wrong, and the test says so. Making `dirty` per scope changes
which scans *trigger* a write; it cannot change that `replace` serialises the
whole `Scoped` map every time. On a cold run every scope is dirty, so it is N
full writes either way —
`a_cold_run_over_several_roots_writes_the_whole_file_once_per_root` pins exactly
that, so nobody re-derives it.

What would work is writing once per process instead of once per scan: `scan`
stops calling `persist`, the assembly layer flushes before it exits. That buys
about 44 MB of writes on a cold run of this repository — under half a second —
and costs a new obligation nobody can see they have forgotten, plus losing the
whole scan rather than part of it when a run is killed. Against a module that
an on-disk index would delete outright, it is not worth it. Written down so the
trade is re-decided rather than rediscovered.

The duplication underneath is the bigger number anyway: `scope` is
`probe@stamp@root`, so a file under `crates/gmr-runtime` is stored once for the
root scope and again for its layer's, and 4.5 MB of distinct content becomes
6.5 MB. No storage format fixes that — see [[survey-cache-scope]]. Making `root`
a filter on the query rather than part of the key does.

## When this changes, ask

Does anything write the cache file from inside the per-file loop again, or
replace the temp-and-rename with a direct write? Either one restores a failure
mode that costs nothing to reintroduce and is invisible until a repository
gets big enough. And does the scan memo still record failures — or has someone
"fixed" it to retry, on the reasoning that a failure should not be cached?

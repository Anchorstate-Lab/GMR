---
about: batteries/survey/src/bridge.rs#Bridge
---

# No dedicated thread — call the async `Index` directly, blocking only where the caller already is synchronous

`Corpus` is sync — it is what `look()` calls. `Index` is async — sqlx and
everything under it. `Bridge` used to cross that boundary with its own
permanent background OS thread (an owned single-threaded `Runtime`, talked
to over an `mpsc` channel, one job at a time). That thread was redundant:
`gmr check`'s only production entry point into `Corpus`
(`InProcess::invoke`, `batteries/transport/src/inproc.rs`) already runs
inside `tokio::task::spawn_blocking` — which is itself the correct,
already-provided way to call blocking code from async context. Bridge's own
thread was a second bridge stacked on top of one that already existed,
serializing every anchor's index calls through one extra OS-thread hop for
no benefit (and, being one job at a time, it never would have let a future
concurrent/networked `Index` backend actually overlap I/O either — found by
profiling a slow `gmr check` with `sample`, which showed ~95% of wall time
in thread-parking primitives rather than actual SQLite execution).

The fix: no owned thread. `run_blocking` picks the right primitive per call:

```rust
pub fn run_blocking<F: Future>(fut: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(fut),
        Err(_) => FALLBACK.with(|rt| rt.block_on(fut)),
    }
}
```

## Handle::block_on vs Runtime::block_on — still real, now scoped to runtime flavor

An earlier version of this file drove jobs through a cloned
`tokio::runtime::Handle` instead of the bridge's owned `Runtime`, and hung
forever on the first real query: `sqlx`'s pool spawns a background
maintenance task on `connect` (governed by `min_connections`/
`idle_timeout`/`max_lifetime`), and for a **current-thread** runtime, only
the *owning* `Runtime::block_on` reliably drives that runtime's
previously-spawned tasks forward together with the future you're blocking
on. A cloned `Handle::block_on` does not enter the same scheduling loop —
the query blocks on a connection permit only the maintenance task would
release, and nothing ever polls it.

This version calls `handle.block_on(fut)` in the ambient branch — the same
shape that hung before — and does not hang, because every real call site now
runs under a **multi-threaded** runtime, not a current-thread one:

- Production: `InProcess::invoke`'s `spawn_blocking` closure runs under
  `main.rs`'s `Builder::new_multi_thread()` runtime. A multi-threaded
  runtime's other worker threads keep polling previously-spawned tasks (the
  sqlx maintenance task included) independently of whatever the calling
  thread's `handle.block_on` is doing — blocking one thread doesn't stall
  the others the way it does when there is only one thread total.
- The `Err(_)` fallback (`registry_uncached()`'s per-call open, plain
  `#[test]` functions with no ambient runtime) uses `FALLBACK.with(|rt|
  rt.block_on(fut))` — `rt` is the *owned* `Runtime`, not a cloned `Handle`,
  and `tokio::runtime::Runtime::new()` defaults to multi-threaded too. Both
  branches sidestep the current-thread-specific gap this section is about,
  not by coincidence but because nothing in this codebase drives `Corpus`
  from a current-thread runtime.

Verified against the real backend, not just tests: `./target/release/gmr
check` run repeatedly against this repo's actual `survey-index.sqlite`
(`max_connections(4)`, WAL, the same pool-with-maintenance-task shape that
hung before) completed correctly every time.

**The risk this section exists to flag has not gone away, only moved**: if
a future caller ever drives `Corpus` (directly, or through a probe) from a
*current-thread* tokio runtime, `run_blocking`'s ambient branch reduces to
exactly the shape that hung before. See "When this changes, ask" below.

## The fallback runtime must be reused, not rebuilt per call

First draft of `run_blocking`'s `Err(_)` branch built a fresh
`tokio::runtime::Runtime::new()` **per call** and dropped it immediately
after. That broke `open_in_memory`'s pool (`max_connections(1)`, no idle
timeout): the pool's one live connection is tied to the runtime that
established it, and an in-memory SQLite database dies with its sole
connection. `Bridge::open` (first call) built runtime A, opened the pool
under it, and returned; runtime A was dropped when `run_blocking` returned;
the very next `Corpus` call (e.g. `refresh`'s `known()`) built a fresh
runtime B with no ambient handle, and sqlx transparently reconnected — to a
brand-new, empty `:memory:` database. Every `tests/bridge.rs` test failed
with `no such table: file`, because the schema-creating `write()` and the
schema-reading `known()` never ran under the same live connection.

Fix: `FALLBACK` is a `thread_local!` `Runtime`, built once per OS thread and
reused for every `run_blocking` call that thread makes — including the one
that opens the pool and every one afterward — for as long as that thread
lives. Each `#[test]` function runs synchronously top-to-bottom on one
thread, so one `Bridge`'s whole lifetime (open, then every `Corpus` call)
shares the same fallback executor and the same live connection.

## `Bridge::open` is `async fn` — the constructor cannot use `run_blocking` itself

`registry()` (`domains/coding/extract/src/lib.rs`) calls `Bridge::open(...)`
directly from inside `async fn served(...)` (`main.rs`), which runs on a
tokio worker thread via `runtime.block_on(run(cli))` — **not** inside
`spawn_blocking`. If `open` blocked internally (the way an earlier draft of
this refactor had it, reusing `run_blocking` for both the constructor and
the six `Corpus` methods), `Handle::try_current()` would succeed there —
same as it does inside `spawn_blocking` — but `handle.block_on(...)` would
panic ("cannot block the current thread from within a runtime that has
spawned it"), because unlike `spawn_blocking`, this call site *is* a worker
thread actively polling the very future that's calling it. There is no
automated test for this path (`domains/coding/cli` has no `tests/`
directory; unit tests use `registry_uncached()` instead) — only running the
built binary surfaces it.

So the constructor does not block at all; it just `.await`s:

```rust
pub async fn open<F, Fut>(tree: impl Into<PathBuf>, open: F) -> Result<Self, IndexError>
where F: FnOnce() -> Fut, Fut: Future<Output = Result<I, IndexError>> { ... }
```

`registry()` (already async) `.await`s it directly — genuinely async-to-async,
no blocking primitive at all. `registry_uncached()`'s closure and
`tests/bridge.rs` (both confirmed-synchronous contexts) wrap the call in
`run_blocking` instead. The rule: blocking-bridge responsibility belongs to
whichever caller is itself synchronous, never to `Bridge::open`, because
`Bridge::open` cannot know in advance which kind of caller it has.

## Generic over `impl Index`, not hardcoded to `SqliteIndex`

The only production backend is `SqliteIndex`, so by "abstract only when
multiple real adapters exist" this could have been concrete. But
`gmr-survey` already runs a stricter rule for `Index` itself: two backends
(`SqliteIndex`, `testkit::Remembered`) must answer the same conformance
suite (see [[survey-index-shape]]). Keeping the bridge generic over
`impl Index` (its own trait bound already requires `Send + Sync` — needed
now that `Arc<Bridge<I>>` is shared across whichever `spawn_blocking` pool
threads happen to run concurrently, not just one dedicated thread the way
it was before) lets the bridge's own translation logic be checked against
the fast in-memory `Remembered` instead of only against real SQLite —
extending the same discipline to the bridge, not adding a new one.
`batteries/survey/tests/bridge.rs` proves this by running `look()` through
both `Surveyed` and `Bridge<SqliteIndex>` and asserting identical answers,
deliberately as a plain `#[test]` rather than `#[tokio::test]` — the whole
point is not needing an ambient runtime.

## `refresh` always calls `write`, even with nothing fresh

`Index::write`'s `INSERT INTO generation` is what opens a generation in the
SQLite backend. A directory that is empty or wholly ineligible for a given
recipe would otherwise never write anything, never open its generation, and
`rows`/`union` would read `None` forever instead of `Some(empty)`. One
`write` call regardless of file count keeps this to a single round trip —
the quadratic per-file write [[survey-cache-write]] measured on the old
`Cache` is exactly the failure shape batching into one `Indexed` slice
avoids.

## When this changes, ask

Does anything call `Corpus` methods, or `run_blocking`, from a
**current-thread** tokio runtime (`Builder::new_current_thread()`, or a
`#[tokio::test]` without `flavor = "multi_thread"`) instead of from inside
`spawn_blocking` on a multi-threaded one? That is the shape of the original
hang — verify against a real backend with a background maintenance task
(`sqlx`'s pool, `max_connections` > 1 or with an idle/lifetime timeout that
spawns a reaper), not just against `testkit::Remembered` which has no such
task, or the regression will pass every test and hang the first time
someone actually uses it against SQLite.

Separately: does anything change `FALLBACK` from a `thread_local!` reused
per-thread back to a runtime built fresh per call? That's the shape of the
"no such table" bug — it will pass a single-call smoke test and fail the
moment a test (or any ambient-less caller) makes a second call against the
same in-memory backend.

---
about: batteries/survey/src/bridge.rs#Bridge
---

# A dedicated thread with its own owned Runtime, not a cloned Handle

`Corpus` is sync — it is what `look()` calls. `Index` is async — sqlx and
everything under it. The bridge crosses that boundary on a background OS
thread that owns a `tokio::runtime::Runtime`, talked to over a channel.

The reason is not "current CLI happens to use multi-thread tokio". A battery
must not assume anything about its caller's runtime: `tokio::task::block_in_place`
reaching into whatever runtime the calling thread happens to be inside would
work for today's one-shot CLI and silently break the day GMR runs as a
longer-lived process, a different domain's assembly, or gets called from a
plain sync test. A fresh `std::thread::spawn` with its own `Runtime` doesn't
care what thread or runtime context calls it — that is the whole reason it
exists.

## Runtime::block_on, not Handle::block_on

The first version drove jobs through a cloned `tokio::runtime::Handle`
instead of the owned `Runtime`. It compiled, spawned the background thread,
and opened the SQLite pool fine — then hung forever on the very first real
query. `sqlx`'s pool spawns a background maintenance task on `connect`
(governed by `min_connections`/`idle_timeout`/`max_lifetime`), and only the
*owning* `Runtime::block_on` reliably drives a current-thread runtime's
previously-spawned tasks forward. A cloned `Handle::block_on` does not: the
query blocks on a connection permit only that maintenance task would
release, and nothing ever polls it.

Found with a minimal reproduction outside the bridge entirely — two
sequential `block_on` calls, no threads or channels — once the real bridge
hung on its first `known()` call. `rt.block_on` twice on the owned Runtime:
fine. `handle.block_on` twice on a cloned Handle, same runtime, same thread:
hangs on the first call. The difference is not "two calls vs one" or
threading at all; it is specifically which handle drives the executor.

## Generic over `impl Index`, not hardcoded to `SqliteIndex`

The only production backend is `SqliteIndex`, so by "abstract only when
multiple real adapters exist" this could have been concrete. But
`gmr-survey` already runs a stricter rule for `Index` itself: two backends
(`SqliteIndex`, `testkit::Remembered`) must answer the same conformance
suite (see [[survey-index-shape]]). Keeping the bridge generic over
`impl Index + Send + 'static` (no `Sync` needed — the wrapped index is only
ever touched by the bridge's own dedicated thread) lets the bridge's own
translation logic be checked against the fast in-memory `Remembered`
instead of only against real SQLite — extending the same discipline to the
bridge, not adding a new one. `batteries/survey/tests/bridge.rs` proves this
by running `look()` through both `Surveyed` and `Bridge<SqliteIndex>` and
asserting identical answers, deliberately as a plain `#[test]` rather than
`#[tokio::test]` — the whole point is not needing an ambient runtime.

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

Does anything call `tokio::runtime::Handle::current()` or hold onto a cloned
`Handle` anywhere near this file? That is the shape of the bug above —
verify against a real backend with a background maintenance task
(`sqlx`'s pool), not just against an operation with no such task, or the
regression will pass every test and hang the first time someone actually
uses it.

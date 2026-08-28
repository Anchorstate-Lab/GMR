---
about:
  - batteries/survey/src/bridge.rs#Bridge
  - batteries/survey/src/bridge.rs#run_blocking
  - batteries/survey/src/bridge.rs#refresh
  - batteries/survey/src/bridge.rs#over_a_still_tree
watch: [sig, logic]
---

# The blocking bridge belongs to whichever caller is already synchronous

`Corpus` is sync — it is what `look()` calls. `Index` is async. `Bridge` is the
one place that crossing happens, and it owns no thread to do it: `run_blocking`
picks the primitive per call.

```rust
pub fn run_blocking<F: Future>(fut: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(fut),
        Err(_) => fallback().block_on(fut),
    }
}
```

The fallback is one `OnceLock` runtime for the process. It was a
`thread_local!`, which made every synchronous caller with no ambient runtime
build a **complete multi-thread tokio runtime of its own**, alive as long as its
thread. A CLI run has one such thread and never noticed. A long-lived process
handing extraction to a worker pool accumulates one runtime per worker and never
gives one back — the fallback for not having a runtime becoming the largest
thing in the process.

Sharing it buys that back and introduces the one risk worth naming: several
threads calling `block_on` on the same runtime at once, which a test covers
because it is the half that could regress without anything failing to compile.

A dedicated thread here would be a second bridge stacked on one that already
exists: the production entry point into `Corpus` is `InProcess::invoke`, which
already runs the extractor inside `tokio::task::spawn_blocking`. That is a
blocking-pool thread, so blocking on it is exactly what it is for.

## `block_on` is legal on a blocking thread and fatal on a worker

`handle.block_on` panics when the calling thread is a runtime worker actively
polling the future that is calling it. Every path into `Corpus` has to arrive
from `spawn_blocking`, or from no runtime at all.

`Bridge::open` is therefore `async fn` and does not block internally.
`registry()` awaits it from `async fn served()`, which runs on a worker thread —
blocking there panics, and no test in the CLI covers it, so only running the
built binary would surface it. Callers that are themselves synchronous wrap the
call in `run_blocking` instead.

The rule is one sentence: **the blocking bridge belongs to whichever caller is
itself synchronous, never to `Bridge::open`**, because `open` cannot know in
advance which kind of caller it has.

## `over_a_still_tree` is a promise the caller makes, not a cache it turns on

The memo is opt-in for a reason that outlives the CLI: it asserts *the tree does
not change while this `Bridge` lives*. A `gmr` run is over in seconds and the
promise holds. A daemon holding one `Bridge` open across edits would keep
answering from a walk of the tree as it used to be, and nothing would say so —
the memo has no way to notice it was lied to.

So a long-lived process must not call it, and the fix for that is not a timer or
an invalidation hook. It is that `Bridge::open` does not install a memo and
`over_a_still_tree` is the caller saying it knows. `registry()` says it because
the CLI is entitled to; a daemon assembling its own registry would not.

## `refresh` memoises the walk, failures included

`over_a_still_tree()` installs a memo keyed by `Generation`, holding the walk's
whole `Result`. One `pass` observes many anchors, and every one naming the same
probe asks for the same walk; without the memo each starts another full-tree
scan on another blocking thread.

**The memo holds only what a re-ask would answer the same way**:
`walked.as_ref().err().is_none_or(Halt::deterministic)`. That is `Ok` and
`Halt::Refused` — a corpus that makes no sense to this recipe refuses the same
way however often it is asked, so surfacing that once with its real reason beats
surfacing it once per anchor.

The other two are not answers, and caching them turns one bad moment into a
permanent one:

- `Spent` is one caller's deadline. The next anchor arrives with a budget of its
  own ([[probe-budget]]), and would be handed a deadline that was never its.
- `Faulted` is the index declining to answer. A lock held for a moment would
  become a refusal for the life of the process.

Both are re-walked, and neither can run away: under a batch deadline every
narrowed budget is already expired once the batch is out of time, so a retry
costs a checkpoint rather than a scan.

The memo is opt-in because it is only sound over a tree nobody is editing.
`registry()` installs it; a registry built per call does not, so callers that
rewrite a file and immediately re-probe — which is what the extractor tests
do — still see the change.

## `refresh` always calls `write`, even with nothing fresh

Writing is what opens a generation. A directory that is empty, or wholly
ineligible for this recipe, would otherwise never open one, and `rows`/`union`
would answer `None` — "no index" — forever instead of `Some(empty)` — "looked,
found nothing". Those are different answers and the barren check in
[[survey-recipe]] depends on telling them apart.

## Generic over `impl Index`

Two backends answer one conformance suite ([[survey-index-shape]]), and keeping
the bridge generic extends that to the bridge's own translation: it can be
checked against the in-memory `Remembered` rather than only against SQLite.

## When this changes, ask

Does anything call `Corpus`, or `run_blocking`, from a runtime worker rather
than from inside `spawn_blocking`? That panics, and the panic is on a path no
unit test here reaches.

Does `Bridge::open` start blocking internally? Its async caller is on a worker
thread, which is precisely where that is fatal.

Does a new `Halt` variant get added without deciding whether it is
`deterministic`? The memo's whole correctness is that predicate, and the
default answer for anything that is not a fact about `(tree, recipe)` is no.

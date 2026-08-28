---
about:
  - crates/gmr-runtime/src/observe.rs#Looked
  - crates/gmr-runtime/src/observe.rs#look_within
  - crates/gmr-runtime/src/observe.rs#looking
  - crates/gmr-runtime/src/observe.rs#looked_at
  - crates/gmr-runtime/src/observe.rs#look_all_within
  - crates/gmr-runtime/src/read.rs#viewed
  - crates/gmr-runtime/tests/operations.rs#one_look_reads_the_log_once_where_asking_twice_reads_it_twice
watch: [sig, logic]
---

# Observing already folds the log; `look` is that fold not being thrown away

Every verb here took a key and rebuilt the world from seq 0. That is right for a
single command and wrong the moment one caller asks two questions about the same
anchor, which is what `check` does on every anchor it has:

```
rt.read(key)      entries(key, 0) -> fold      the view the audit needs
rt.observe(key)   entries(key, 0) -> fold      the same entries, again
```

Measured on this repository — 509 anchors, 60k entries — the first pass cost
548ms and the second 696ms of a 4.3s run. Not a constant factor on a slow part:
**a quarter of `check` was re-reading what it had just read.**

`observe_with` folds an `AnchorState` and drops it on the floor at the end,
returning only what the state *did*. `Stood` is that state kept, and `Looked` is
it rendered. One journal pass now answers both questions, and the counting
journal in the test is what fails if a third pass creeps back.

## `before`, not `after`, and this is not a preference

The obvious shape is to hand back the anchor as it stands *after* the look. It
is wrong here, and one caller proves it: `swapped` reports an instrument change
by comparing `view.derivation` — the version recorded by the last observation —
against what this build resolves live. Observe first and the record **is** the
live version, so a post-observation view can never report a swap. The signal
would not become noisy; it would become silently, permanently empty.

So `Looked.before` is the state as of just before the probe ran, and
`scheduler.seen` is read before observing too, because `sighted` moves the
count and the date this call is about to write.

## Why `Observed` did not simply grow fields

The first shape was to add facts to `Observed::Transitioned` and friends. Three
callers wanted three different things — `check` the facts, `pass` the anchor's
terminal set, the `observe` verb the transitions — so `Observed` would have
grown until it was an `AnchorView` with a worse name. `Observed` keeps
answering exactly what [[runtime-observed]] says it answers: what this one
observation did. Which anchor it happened to is a second question, and `Looked`
is where the two are carried together.

## `pass` stopped re-folding to ask one boolean

`pass` decided retirement with `fold(&log.entries(anchor, 0))` *after*
observing, to ask `is_terminal(to)`. An observation appends `Transition`,
`Still` or `Attempt`; only `Revise` can change the anchor's declaration, and
`pass` does not revise. The pre-observation anchor answers the same question,
so a whole journal pass per moved anchor bought nothing.

## What this does not fix

The remaining serial cost is the probing itself — 1.17s of that 4.3s, one
anchor at a time. `Budget`'s cancellation chain is thread-safe and
`InProcess::invoke` already goes through `spawn_blocking`, so the work is off
the async workers and would genuinely parallelise. That is a separate change
with its own question — a batch mints one `Budget` ([[runtime-pass-skipped]]),
and what that means when the batch runs at once has to be answered before, not
after.

## When this changes, ask

Does a caller start deriving anything from `Looked.before` that observation can
change? It is a snapshot from before the probe ran, and the further it travels
the easier it is to read it as "now".

Does something want the view *after* the look? That is a real question with a
real cost — a second fold, or a seeded one — and it must not be answered by
quietly moving `viewed` past the observation, which is the `swapped` bug above
arriving through the back door.

## Looking at many is one call, and the concurrency lives here

`look_all_within` is the same look, bounded by `Policy.observe_at_once`. It is in
the runtime rather than in `check`'s loop because observing a *set* of anchors is
orchestration, and M3 plans two more shells — an MCP server and SDK adapters — that
would each otherwise write this loop again, each with its own idea of how many at
once and whether order survives.

**Order survives, and that is load-bearing.** `check` zips these answers against
the keys it asked about and prints one line per pair, so a reordered batch
attributes one anchor's drift to another — silently, and worse the more anchors
there are. That is `buffered`, not `buffer_unordered`, and the difference is one
identifier with no compiler between it and a wrong report.

A test pins it, and the first version of that test **did not**: five in-memory
anchors complete in poll order, so it passed under `buffer_unordered` too. It
earns its keep now only because the journal under it is made to answer out of
order on purpose, slowest for the key asked first. A concurrency test that cannot
observe concurrency is a test of nothing.

## The threads are not ours, and that is why there is still no tokio here

`futures_util::buffered` is a combinator: it polls futures already in hand, on
whoever's runtime is driving. It picks no executor, so [[layers]]' entry test for
this crate is untouched and the property M1 leans on — `gmr-runtime` depends on no
runtime, which is what makes the daemon a deployment rather than a rewrite — still
holds.

The parallelism that results is real anyway, and it was already paid for: the
in-process extractors run under `spawn_blocking` in the transport battery, so
sixteen concurrent looks become sixteen extractions on tokio's blocking pool.
**The runtime supplies the concurrency and the battery supplies the threads.**
Measured here: the look loop went 3.08s to 1.59s and `check` from ~3.4s to ~2.1s,
with CPU crossing 100% for the first time.

What is left is not this loop. Roughly 0.7s of every run is fixed startup —
loading the extract cache, the catalog, the subscriptions — and it is paid whether
one anchor is checked or all 528. That is the next thing worth measuring, not more
concurrency.

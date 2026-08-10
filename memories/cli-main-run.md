---
about:
  - domains/coding/cli/src/main.rs#run
  - domains/coding/cli/src/main.rs#served
  - domains/coding/cli/src/main.rs#main
watch: [logic]
---

# Some verbs are handled before a `Runtime` exists, because they must not touch it

`run` dispatches `Publish`, `Probes`, and `Init` before the journal is even
opened: publishing an artifact happens before any log exists, and building
probes likewise never touches the journal — routing them through a full
`Runtime` would be pure overhead for verbs that only need the filesystem
and the probe store.

## Why the split into `run` and `served`

`run` stops as soon as the store is open; everything downstream of that
lives in `served`. The split exists so there is exactly one place that
closes the store, on every path a store was opened on, including the error
ones. Inlining `served` back into `run` means either duplicating
`store.close().await` at each `return`, or leaking the pool on the paths
that forget — and the pool is closed explicitly because `main` no longer
waits for it, see below.

`main` builds the runtime by hand rather than through `#[tokio::main]`, and
ends with `shutdown_background()`. Dropping a runtime blocks until every
blocking task that has already started finishes, and a `spawn_blocking`
closure cannot be cancelled from outside — so an extractor that overran its
budget kept a core pegged for minutes *after* the CLI had already printed
its error and `run` had returned. `shutdown_background` detaches those
threads so they die with the process. `ExitCode` is returned rather than
calling `std::process::exit`, which would skip std's stdout flush and
silently drop buffered output whenever stdout is a pipe — which is how CI
reads it.

`Export` and `Import` are dispatched first inside `served`, before
`Runtime::builder().build()` — deliberately on the raw
`SqliteStore`, not through a `Runtime`. A schema-version gap between the
export's format and this build is exactly the situation `import_jsonl`
(see [[store-portable-import]]) has to detect and refuse; constructing a
full `Runtime` around a store that might not even open cleanly would be
the wrong move for a verb whose whole point is moving data across that
boundary.

Only after those five verbs are ruled out does `run` build the three
`Transport`s and wire the `Runtime` — one router in front of the extractors
linked into the binary (`InProcess`), a user's own script file
(`Script`), and a published, exec'd artifact (`Shell`) — see
[[runtime-assembly]] for why the resulting `Runtime` stays split by
capability rather than becoming one grab-bag struct.

`ClaudeMemory::new` failing is read-only and additive, not fatal: no
Claude Code session running outside `~/.claude/projects/...` is a normal,
expected absence, not a misconfiguration. The failure is recorded on the
builder via `provider_warning` (see [[runtime-provider-warning]]) rather
than only `eprintln!`'d, so a `--json` caller (or `gmr doctor`) has a way
to learn about it too.

## When this changes, ask

Does a new verb that never touches the journal still get dispatched only
after `Runtime::builder().build()`? That would pay for state directory
creation and store opening it does not need. And does any new provider
failure at startup only go to stderr, with no path onto the builder as a
warning?

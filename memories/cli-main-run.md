---
about:
  - domains/coding/cli/src/lib.rs#run
  - domains/coding/cli/src/lib.rs#served
  - domains/coding/cli/src/main.rs#main
watch: [logic]
---

# Some verbs are handled before a `Runtime` exists, because they must not touch it

`run` and `served` live in the **library**; `main.rs` holds only `main`. The
line between them is what a second front end would want: everything from parsing
onward is callable, and what stays behind is the part that is only true of a
terminal — building a tokio runtime by hand, and turning an outcome into an
`ExitCode`. See [[cli-embeddable]] for what that unblocked and the test that
keeps it unblocked.

`run` dispatches `Publish`, `Probes`, `Init`, and `Adopt` before the journal
is even opened: publishing an artifact happens before any log exists, building
probes never touches the journal, and `adopt` nominates from a repository that
may not have run `init` yet — cold start is its whole point, so requiring a
`Runtime` would gate the door on the thing behind the door. Routing any of
them through a full `Runtime` would be pure overhead for verbs that only need
the filesystem and the probe store.

## Why the split into `run` and `served`

`run` stops as soon as the store is open; everything downstream of that
lives in `served`. The split exists so there is exactly one place that
closes the store, on every path a store was opened on, including the error
ones. Inlining `served` back into `run` means either duplicating
`store.close().await` at each `return`, or leaking the pool on the paths
that forget — and the pool is closed explicitly because `main` no longer
waits for it, see below.

`main` builds the runtime by hand rather than through `#[tokio::main]`, and
ends with `shutdown_background()`. That is the one piece a library must not do
for its caller: an embedder has its own runtime and its own idea of when to stop. Dropping a runtime blocks until every
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

Which stores get wired up is a list, not a sequence of branches, and it
lives in `stores::assembled` rather than here. Each battery hands back a
`MemoryStore` — the one contract plus whichever capabilities that backend
has — so `served` only has to hang `content()` on the builder and turn a
construction failure into a warning. See [[content-discovery]] for why
"can this store be enumerated" travels as a value.

It was three hand-written branches, each a different shape: git
unconditional, `ClaudeMemory` a `Result`, mem0 an env guard around a
`Result`. Adding a fourth backend meant copying the third and renaming it,
and the difference between the shapes carried no meaning — only the order
they were written in.

What each of them decides still matters and is unchanged:

- A store that fails to construct is read-only and additive, not fatal. No
  Claude Code session running outside `~/.claude/projects/...` is a normal,
  expected absence, not a misconfiguration. The failure is recorded on the
  builder via `provider_warning` (see [[runtime-provider-warning]]) rather
  than only `eprintln!`'d, so a `--json` caller (or `gmr doctor`) has a way
  to learn about it too.
- **Naming no mem0 registers nothing and warns about nothing** — not using
  mem0 is not a misconfiguration, and a warning on every run for a store
  the reader has never heard of is noise that teaches people to stop
  reading warnings. A mem0 that *is* named and then fails to build warns
  like any other. The line is "did the person ask for this store", and only
  an env var can answer it.
- Two env vars answer it, because there are two mem0s. `MEM0_BASE_URL`
  means a self-hosted server and selects that dialect; `MEM0_API_KEY` alone
  means the managed platform. A self-hosted server run with `AUTH_DISABLED`
  has no key at all, which is why registration cannot be gated on the key
  the way it once was, and why the key is optional on that branch and
  travels in a different header — see [[provider-mem0]].
- `MEM0_BASE_URL` used to mean "the platform dialect, pointed somewhere
  else", which is the one thing a reader setting it never means. Someone
  who sets it is running their own server, and that server mounts different
  routes; the old reading reached none of them while reporting that
  somebody else's service was unavailable.

`served` keeps the assembled list after building the runtime for two reasons.
`gmr memories` dispatches over the stores that can list — the `Runtime` holds
providers, not sources, and [[content-discovery]] says why. And `stores.names`
is the one book of names, minted where every source is already in hand and
handed down from here into every verb that prints a record.

That it is *handed down* rather than constructed per verb is the point. A verb
that takes `root` and builds a declaring source of its own makes "look the name
up" a choice at every call site, and skipping it costs nothing: `addressed()` is
always available, always the right type, compiles, and prints like a name. What
a reader sees then depends on which verbs happen to hold a `root` rather than on
anything anybody decided. There is no route by which a verb can mint one. See
[[cli-notes-source]] for why naming belongs to the address and to this domain.

Registering mem0 means this binary links `reqwest`. That is why
`gmr-provider` keeps `mem0` off by default and this crate turns it on:
the battery stays free of io in its default feature set — which
`architecture.toml`'s `forbidden_default` mechanically checks — while the
shipped binary, whose whole job is to be assembled for a domain, carries
it. See [[provider-mem0]].

## When this changes, ask

Does a new verb that never touches the journal still get dispatched only
after `Runtime::builder().build()`? That would pay for state directory
creation and store opening it does not need. And does any new provider
failure at startup only go to stderr, with no path onto the builder as a
warning?

Does a new backend get wired up here instead of in `stores::assembled`? A
branch here is the shape that was just removed, and the next one after it
will be a copy of this one.

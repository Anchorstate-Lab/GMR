---
about: domains/coding/cli/src/main.rs#run
watch: [logic]
---

# Some verbs are handled before a `Runtime` exists, because they must not touch it

`run` dispatches `Publish`, `Probes`, and `Init` before the journal is even
opened: publishing an artifact happens before any log exists, and building
probes likewise never touches the journal — routing them through a full
`Runtime` would be pure overhead for verbs that only need the filesystem
and the probe store.

`Export` and `Import` are dispatched right after the store opens, but
still before `Runtime::builder().build()` — deliberately on the raw
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

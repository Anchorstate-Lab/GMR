---
about:
  - console/cli/src/lib.rs#probes_dir
  - console/cli/src/lib.rs#stale_journal_guard
watch: [sig, logic]
---

# The probe store sits inside the journal's directory, and a moved journal is never silently re-created

`probes_dir` colocates the content-addressed artifact store with the
journal under the same root — one place to find both, not two separately
configured paths.

Both live in the library rather than the binary, and that is where a guard about
opening a store belongs: an embedder opens the same journal through the same
`run`, so a guard only the terminal ran would protect only the terminal. See
[[cli-embeddable]].

`stale_journal_guard` exists because the journal's location moved (from
`.anchor/memory.db` to `state/memory.db`) at some point in this project's
history. If `run` just opened whatever database it found at the new path,
a repository still carrying the old `.anchor/memory.db` would silently
start a fresh, empty journal at the new location — erasing access to
history that nothing else can rebuild, since the journal is the one place
that history lives. Refusing loudly and telling the user exactly which
files to move (including the `-wal` file, which can hold entries not yet
flushed into the `.db` itself) is the only safe response to finding the
old file still present.

## When this changes, ask

Does a future journal-location change get handled by silently opening (or
creating) a database at the new path? Any migration that can leave an old
journal unnoticed needs its own guard like this one, checked before the
new path is ever opened.

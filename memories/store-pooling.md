---
about:
  - crates/gmr-store/src/sqlite/mod.rs#Pooling
  - crates/gmr-store/src/sqlite/mod.rs#open_with
  - crates/gmr-store/src/sqlite/mod.rs#connect
watch: [sig, logic]
---

# Four connections is a CLI's answer, and it was the only answer on offer

`connect` held `max_connections(4)` and a five-second `busy_timeout` as
literals. Both are right for the shipped CLI — one process, a handful of
concurrent reads, a run measured in seconds — and neither is a fact about
SQLite or about this schema. A long-lived server sharing one fact layer across
agents wants a wider pool and its own patience; it had no way to say so short of
forking the function.

So they are a `Pooling`, `open` is `open_with(path, Pooling::default())`, and
the default is exactly what was hard-coded. Nothing about the shipped binary
changed.

## Why it is not in `Policy`

[[anchor-RunSettings]] and `Policy` are about what the runtime *does* — how
often to look, how long a probe may take. This is about how a particular
backend's connections are managed, and it is chosen at `open`, before a
`Runtime` exists at all. Routing it through `Policy` would put a SQLite detail
in a type the runtime hands to every deployment, including ones with no SQLite
in them.

## `open_in_memory` keeps its own numbers

One connection, no idle timeout, no max lifetime — and those are not tuning.
An in-memory SQLite database *is* its connection: let the pool open a second one
and it is a second, empty database; let it retire the first and the data is
gone. They are the shape of the thing, so they stay literals rather than
becoming a `Pooling` somebody could set wrong.

## When this changes, ask

Does a knob arrive that changes what the store *means* rather than how it is
reached? Journal mode and foreign keys are in `SCHEMA` because a database
carries them; these are per-process and carry nothing.

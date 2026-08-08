---
about:
  - crates/gmr-runtime/src/memory.rs#ProviderWarning
  - crates/gmr-runtime/src/memory.rs#provider_warnings
watch: [sig, logic]
---

# A provider warning is assembly-time, and its name is not a promise

`ProviderWarning` records that the domain *tried* to register a provider
and couldn't — that happens once, at assembly, not per fetch, so it does
not belong in `ContentError` (which is a per-operation failure type).
`provider` is whatever name the domain was attempting to register under; it
is not guaranteed to be a name that ever made it into
`MemoryLens.providers`, since registration is exactly what failed.

`provider_warnings()` exists so a `--json` caller has a way to learn about
this that is not "read stderr" — a battery failing to construct at startup
would otherwise only ever be visible to whoever was watching the terminal.

## When this changes, ask

Does a new failure mode belong here (assembly-time, one-shot) or in
`ContentError` (per-fetch, recurring)? Conflating the two would either lose
a fetch-time error's detail or make a one-time construction failure look
like it recurs on every call.

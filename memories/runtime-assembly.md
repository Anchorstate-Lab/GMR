---
about:
  - crates/gmr-runtime/src/assembly.rs#Runtime
  - crates/gmr-runtime/src/assembly.rs#provider_warning
  - crates/gmr-runtime/src/log.rs#AnchorLog
  - crates/gmr-runtime/src/memory.rs#MemoryLens
  - crates/gmr-runtime/src/observer.rs#Observer
  - crates/gmr-runtime/src/scheduler.rs#Scheduler
  - crates/gmr-runtime/src/scheduler.rs#settings_for
watch: [sig, logic]
---

# `Runtime` splits into four services so a new verb inherits no capability by default

`Runtime` is deliberately not one struct with every store injected into it;
it is a facade over `AnchorLog`, `Observer`, `MemoryLens`, and `Scheduler`,
each holding only the stores relevant to what it does. A verb module takes
whichever of these four it needs as a parameter, rather than a handle to
the whole `Runtime` — that way adding a new verb never accidentally gives
it the ability to touch, say, the queue when all it needed was the
journal. `AnchorLog` is the smallest of the four: it wraps only a
`Journal`, with no probe, no bindings, no queue — a verb that only reads or
appends log entries cannot reach any other store through it, even by
accident. `MemoryLens` is the mirror case: bindings, seals, links, and
content providers, but no journal, no transport, no queue — a verb dealing
with bound content cannot reach the log or the scheduler through it.
`Observer` holds only the wired-up `Transport`s — no journal, no bindings,
no queue — so anything that only needs to resolve or invoke a probe cannot
reach any store through it either. `Scheduler` holds the queue, the
deployment's policy numbers, and the per-anchor settings that override
them; its queue is `Option`al because a deployment without one is legal —
in that case the lease-based observe path (see [[store-queue-fence]]) is
simply unavailable, not an error. `settings_for` returns whatever was set
for the anchor or falls back to the deployment default, and is safe to
call with no rationale on either path because `RunSettings` is never
sealed criteria — see [[anchor-RunSettings]].

`provider_warning` exists because a `ContentProvider` a domain tried to
construct but couldn't (missing `$HOME`, missing config, whatever) should
not just print to stderr and vanish. Recording it on the builder means it
survives into the built `Runtime` and becomes queryable — `gmr doctor` can
report it — instead of only ever being visible to whoever happened to be
watching the terminal at construction time.

## When this changes, ask

Does a new verb receive a full `&Runtime` instead of the specific services
it touches? That reopens exactly the capability-by-default problem the
four-way split exists to avoid.

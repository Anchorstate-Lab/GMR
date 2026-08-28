---
about:
  - crates/gmr-runtime/src/assembly.rs#Runtime
  - crates/gmr-runtime/src/assembly.rs#build
  - crates/gmr-runtime/src/assembly.rs#try_build
  - crates/gmr-runtime/src/assembly.rs#AssemblyError
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
accident. It also holds the one piece of mutable state in this crate: a
checkpoint of each anchor's folded state, so `state()` and `stood()` ask the
journal only for the entries appended since they last looked. That is not a
second store and reaches nothing new — it is derived from the journal it already
wraps, and throwing it away changes only how long a fold takes. It is safe to
keep with **no invalidation at all** because the journal is append-only: a state
at a seq can go stale, never wrong, so catching it up is extension and never
correction. `append` deliberately does not write to it — extension is the only
operation the cache has, and a second writer is how an invalidation bug gets in.
Two runtimes over one journal, holding checkpoints of different ages, fold to
byte-identical states; a test pins that, because it is the property that makes
this safe to leave per-process rather than shared. `MemoryLens` is the mirror case: bindings, seals, links, and
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

## `build` refuses two providers under one name

Lookup finds a provider by scanning the registered list for a matching
`ProviderId` and taking the first hit. Register two under one name and
every reference through that name resolves against one of them and never
the other — silently, forever, with no signal anywhere. So `build` asserts
the names are distinct.

`build` panics on it, and on a missing store beside it: both are mistakes in
how a binary was assembled, decided before anything runs and unfixable at
runtime by the caller. Making *every* assembly site branch on a condition none
of them can recover from is the cost that argument refuses, and it still holds
for a binary that wrote its own assembly in Rust.

It stops holding the moment the assembly comes from a file somebody wrote. A
service reading a configuration has a caller who can act — fix the line — and a
panic there is a stack trace where a sentence naming the bad line belongs. So
`try_build` returns `AssemblyError`, and `build` is `try_build` with the error
raised as a panic carrying the same sentence. One definition of what a complete
`Runtime` is, two ways of being told.

`Part` is an enum rather than a `&'static str` for the ordinary reason: a caller
that wants to say "you forgot the queue" in its own words needs to match on
which part, and a string forces it to match on prose that was written to be read
by a person.

Distinct names are worth having because a `ProviderId` is an **instance**
alias, not a type name — one binary can reach two mem0 accounts, or a
hosted store and a self-hosted one. The shipped CLI does not expose that
yet, deliberately: an alias is written into bindings, bindings are
append-only, and the verb that would move them to a new name is not built.
Handing out a user-settable alias before that exists would be handing out a
one-way door. See [[provider-mem0]].

## When this changes, ask

Does a new verb receive a full `&Runtime` instead of the specific services
it touches? That reopens exactly the capability-by-default problem the
four-way split exists to avoid.

Does a new required part get added to the builder without a `Part` variant? Then
`build` still panics correctly and `try_build` starts lying by omission — the
error can only name what the enum can say.

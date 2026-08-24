---
about:
  - domains/coding/cli/src/memories.rs#located
  - domains/coding/cli/src/memories.rs#registered
watch: [sig, logic]
---

# What an address means is settled by grammar; whether it can be reached is settled by the registry

`located` does two things in that order and never lets the second decide the
first:

```
parse     split the first colon; is the prefix a legal ProviderId?
          yes → this text is an address     no → this text is an id in the default store
resolve   is that store registered in this run?
          yes → the Ref                     no → refuse, and name what is registered
```

Letting the registry decide the *parse* is what this replaces, and it failed
in the way [[layers]] exists to prevent. `mem0:9f8e` was an address wherever
mem0 happened to be configured and a git path everywhere else — so one text
named two different records, and which one depended on an environment
variable that has nothing to do with the record.

## Refusing is the only honest answer, because the fallback laundered a failure

Not knowing a store is **our** failure. Rewriting the address to the default
store hands it to git, which answers truthfully that no such path exists —
and that is **the world's answer**, `Gone`, a state no reader can tell from a
record somebody really deleted. `doctor` then tells them to restore it or let
it go, about a record that never existed.

Two things make it worse than the usual misdiagnosis. A probe recomputes
every round, so a wrong reading is transient; a binding is written once and
nothing ever re-derives it. And the binding table only grows — the laundered
row is permanent, and `cobound` will list it beside the real ones forever.

## The grammar lives on `ProviderId`, not in this file

What a store's name may look like is the type's business
([[core-newtype-classes]]), so `located` asks `ProviderId::try_new` instead of
carrying its own idea of a prefix. That keeps the answer identical in every
build, and it keeps the old rule's *reason* — an id may contain a colon —
which now holds because `memories/a:b.md` has a `/` in its first segment and
cannot be a provider name, not because some registry was consulted.

`--provider` still wins where it was always meant to: it names the store, so
the whole text is the id and colons in it survive untouched.

## When this changes, ask

Does any path here fall back to a store the caller did not name? That turns
"we could not resolve this" into "that store says it is not there", and the
binding recording it cannot be taken back.

---
about:
  - domains/coding/cli/src/stores.rs#assembled
  - domains/coding/cli/src/stores.rs#Where
  - domains/coding/cli/src/stores.rs#take
watch: [sig, logic]
---

# Adding a store to this binary is a line in a list, not a branch in a function

`assembled` walks `REGISTERED`, a slice of `(name, fn(&Where) -> Made)`.
Every store — including `git`, which is this domain's own notes — is one
entry, because the moment one of them is special-cased the list stops being
the answer to "what can this binary read" and becomes a partial one.

`Where` is what the domain can hand a store at assembly: the repository
root, and the notes source `git` gives its listing. Passing it to every
entry rather than only the one that reads it is what keeps the entries the
same shape.

## `Made` spells three different answers, and the middle one is not a fault

```
None            not configured here          silent, and correct
Some(Err(e))    configured and unbuildable    a warning, and red
Some(Ok(store)) built
```

mem0 with no key set is `None`: not using mem0 is not a misconfiguration,
and warning about it would train the reader to ignore the channel that
carries real faults. mem0 with a key and a bad scope is `Some(Err)` — the
owner asked for this store and it could not be built.

The warning channel is the same one `doctor` reports as `provider_warnings`
and the runtime registers with `provider_warning`, so a store that failed
to build produces "no provider named `x` is registered in this binary" at
the point of use rather than a silent absence.

## Providers declared in a recipe are not in the list

`REGISTERED` is what this *binary* can be; `.anchor/providers.toml`
([[cli-providers-recipe]]) is what this *repository* declares. They are
assembled one after the other and land in the same `built` vector, but they
answer different questions and a recipe must never need a rebuild to be
read.

## When this changes, ask

Does a new store need something `Where` does not carry? Add it there rather
than lifting that one store out of the list — the exception is what the list
exists to prevent.

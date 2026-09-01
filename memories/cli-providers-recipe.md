---
about:
  - console/cli/src/providers.rs#declared
  - console/cli/src/providers.rs#assembled
  - console/cli/src/providers.rs#script
  - console/cli/src/providers.rs#can
  - console/cli/src/providers.rs#caveat
watch: [sig, logic]
---

# `.anchor/providers.toml` is `probes.toml` for stores, deliberately

The loader is the same shape as [[cli-probes-recipe]]'s — a `File` struct
with one `BTreeMap`, a missing file reads as no declarations, a malformed
one is an error naming the path. Copying that shape is the point: an owner
who has written one of these files has written both.

A missing file is not a fault. A repository that declares no store of its
own is the ordinary case, and treating silence as an error would make the
file mandatory for everyone to omit correctly.

## One transport per provider, not one namespace for all of them

Each declared provider gets its own `Script` transport holding at most two
entries, named `fetch` and `list`. The alternative — one transport keyed by
`<provider>-<what>` — puts every provider's scripts in one namespace with
the probe scripts, where `ProbeName`'s alphabet (lowercase, digits, `-`)
gives no separator that a provider name cannot itself contain.

## A script that is not there is refused at assembly

`script` checks the file exists while the store is being built, not when it
is first read. A store whose script is missing fails every read, and a
failed read is `Unreachable` — which by design never turns `doctor` red,
because it means somebody else's service is down. A recipe pointing at a
file the owner can create must not be reported as somebody else's outage.

That refusal arrives as a `Warning`, so it goes through the same channel
mem0 and claude-code already use: named in `doctor`'s `provider_warnings`,
printed at startup, and red. A malformed *file*, by contrast, stops the
command — one store being unbuildable is a fault about that store, while a
file that will not parse means nothing in it was read.

## What a store can do is declared, and said before it is asked

`ids` is required, with no default. Whether a store's ids are ones a person
could write down is a fact only the owner knows, and guessing it produces
confident wrong advice: guessing `readable` tells someone to name a record
they can never name, and guessing `opaque` tells them to give up on one they
could have named. One required word costs less than either.

`version` accepts the value it is forced to have and refuses the other one
by name. A recipe asking for `native` is refused at load with the reason —
there is no channel in `Transport::invoke` for the store's own revision (see
[[provider-declared]]) — because the alternative is an owner who believes
their store's revisions are being tracked and never finds out otherwise.

`caveat` fires on the one combination that closes off every way in: opaque
ids and no listing means nothing can enumerate the store and nothing can
name a record in it, so only memories bound at the moment they are written
can ever be anchored. Each half alone is fine, which is why this is computed
from the pair rather than warned about per key.

`doctor` says all of it at assembly. The failure this replaces is finding
out what a store cannot do by watching a command fail — where every answer
looks like the store being down.

## When this changes, ask

Does a second thing in the recipe become optional? Every optional key is a
capability the store may or may not have, and the answer belongs where
`list` already puts it: a `MemoryStore` that does not offer the trait, not a
trait that answers "I have none".

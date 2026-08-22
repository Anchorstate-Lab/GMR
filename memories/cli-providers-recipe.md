---
about:
  - domains/coding/cli/src/providers.rs#declared
  - domains/coding/cli/src/providers.rs#assembled
  - domains/coding/cli/src/providers.rs#script
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

## When this changes, ask

Does a second thing in the recipe become optional? Every optional key is a
capability the store may or may not have, and the answer belongs where
`list` already puts it: a `MemoryStore` that does not offer the trait, not a
trait that answers "I have none".

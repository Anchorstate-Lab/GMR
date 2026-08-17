---
about:
  - batteries/provider/src/mem0/mod.rs#Mem0
  - batteries/provider/src/mem0/mod.rs#version_of
  - batteries/provider/src/mem0/mod.rs#absent
  - batteries/provider/src/mem0/mod.rs#fetch_at
  - batteries/provider/src/mem0/mod.rs#list
  - batteries/provider/src/mem0/http.rs#Http
watch: [sig, logic]
---

# The first store GMR reads that it does not also own

This is the backend the three-tier provider contract was written against,
and the first one that is remote, mutable and someone else's. Four choices
here are the load-bearing ones.

## The version is a hash of the text, not `updated_at`

mem0 has no notion of a version at all — it has a memory, and a change log.
Something has to play `Version`, and the two candidates were `updated_at`
and a hash of the memory text. The hash wins on both halves of the
invariant every provider owes: *same content ⇒ same version* holds by
construction, where `updated_at` would make an untouched memory look
rewritten the moment mem0 touched a timestamp; and *content changed ⇒
version changed* holds without depending on mem0's timestamp resolution,
where two updates inside one millisecond would be indistinguishable.

It also makes `fetch_at` exact. mem0 has **no endpoint that returns a
memory as of a version** — the plan for this work assumed one and was
wrong. What it has is `/history/`, an append-only log of what each change
produced, and hashing each `new_memory` finds any version the memory ever
held without a timestamp comparison anywhere.

## A 404 is three different facts, and only one of them is `Gone`

mem0 answers 404 for a memory that was deleted, for a key that lost its
permission, and for a scope that no longer matches. `absent` therefore does
not map 404 straight to `Ok(None)`: it asks mem0 to list the configured
scope, and only a listing that works makes the 404 authoritative. A listing
that fails turns the whole thing into `Err`.

This costs one extra call, on a path that is rare, and it buys the
difference between "this record is gone" and "you cannot see this record
from here". Getting it wrong is not a cosmetic error: `doctor` would print
a screenful of dead references that all still exist, and the obvious repair
a reader would reach for is to delete those bindings.

## Every record is `Silent`, including ones carrying `metadata.gmr`

mem0 has a metadata bag, and reading a `gmr` key out of it would be easy.
It is deliberately not read. Doing so would advertise a declaration channel
mem0 makes no promise about — its update path says nothing about metadata
surviving — and a channel that works today and quietly stops tomorrow is
worse than one that never existed. Declarations for stores like this go
through `gmr bind`, which is the base primitive anyway; see
[[content-discovery]] for why `Claim` is a source's option rather than a
record's duty.

## Never writing is a property of the seam, not a rule anyone keeps

The `Http` trait this module talks through has `get` and nothing else, so
there is no method here that could write into somebody else's store. That
is why the guarantee needs no test: the alternative would not compile.

## What a fake can and cannot check

Everything decidable without a network — version derivation, history
reconstruction, the 404 split, listing, pagination, budget exhaustion — is
tested against `Canned`. What is left is whether mem0 still answers in the
shape this module reads, and no fake can know that; that is
`tests/mem0_live.rs`, `#[ignore]` and driven by real credentials. It is a
canary for API drift, not a test of this crate's logic, and treating it as
the latter would mean the logic goes untested whenever the key is absent.

The JSON structs here take `#[serde(default)]` on everything they can,
because this module reads a service it does not control: a field that
appears or disappears should not break a version derivation that never
looked at it.

## When this changes, ask

Does a write method appear on `Http`? That is the one guarantee this
battery exists to keep, and it is currently kept by there being nothing to
call.

Does anything start deriving a version from something other than the text —
a timestamp, an id, an ETag? Each of those reintroduces the failure this
hash was chosen to avoid, and the failure is silent: bindings read as
rewritten when nothing was rewritten.

---
about:
  - batteries/provider/src/mem0/mod.rs#Mem0
  - batteries/provider/src/mem0/mod.rs#Deployment
  - batteries/provider/src/mem0/mod.rs#Absence
  - batteries/provider/src/mem0/mod.rs#self_hosted
  - batteries/provider/src/mem0/mod.rs#whole
  - batteries/provider/src/mem0/mod.rs#version_of
  - batteries/provider/src/mem0/mod.rs#absent
  - batteries/provider/src/mem0/mod.rs#fetch_at
  - batteries/provider/src/mem0/mod.rs#list
  - batteries/provider/src/mem0/http.rs#Http
watch: [sig, logic]
---

# The first store GMR reads that it does not also own

This is the backend the three-tier provider contract was written against,
and the first one that is remote, mutable and someone else's.

## There are two mem0s behind one name

The managed platform at `api.mem0.ai` and the self-hosted server in mem0's
own `server/` are not the same service wearing different hostnames. They
disagree about three things, each of which is silent when got wrong:

```
                platform                     self-hosted
route           /v1/memories/{id}/           /memories/{id}
                (trailing slash required)    (trailing slash is a 404)
key travels in  Authorization: Token …       X-API-Key: …
"no such one"   404                          200 with a body of `null`
```

The server runs FastAPI with `redirect_slashes=False`, so the platform's
trailing slash is not redirected to the route next door — it is a 404. All
four addresses this battery builds miss all four routes.

That was measured, not reasoned about: a self-hosted server, stood up in
docker with a memory and a change log seeded into it, answered 404 to every
platform-shaped URL and 200 to every self-hosted-shaped one.

**`Deployment` exists because the alternative was invisible.** Aimed at a
self-hosted server, the platform dialect produced: a listing that failed, a
`fetch` that blamed the operator's credentials, and — worst — a `fetch_at`
that returned `Ok(None)`, which the runtime reads as `Before::NotRetained`,
a wholly ordinary condition. Two of those three land in `Unreachable`,
which by D6 never turns `doctor` red. So the misconfiguration produced a
permanently green report saying somebody else's service was to blame. A
provider that cannot reach its store at all must not be able to say that.

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
wrong. What it has is a change log, an append-only record of what each
change produced, and hashing each `new_memory` finds any version the memory
ever held without a timestamp comparison anywhere. Both deployments keep
that log, which is why one `History` implementation serves both.

## Only one of the two absences needs a second call

A platform 404 is three different facts: a memory that was deleted, a key
that lost its permission, and a scope that no longer matches. `absent`
therefore does not map it straight to `Ok(None)` — it asks mem0 to list the
configured scope, and only a listing that works makes the 404
authoritative. That costs one extra call on a rare path, and it buys the
difference between "this record is gone" and "you cannot see this record
from here". Getting it wrong is not cosmetic: `doctor` would print a
screenful of dead references that all still exist, and the repair a reader
would reach for is to delete those bindings.

**The self-hosted `200 null` needs no such probe, and adding one would be a
round trip that can decide nothing.** Nothing else produces that answer: a
rejected key is a 401, and a store that cannot answer is a 502. This is why
`Absence::Unconfirmed` carries the probe URL rather than the caller
deciding whether to probe — the dialect that has no probe cannot construct
the variant that needs one, so there is no reachable path where the wrong
URL gets asked.

The same asymmetry runs the other way for history. A platform 404 there
means the memory is gone and took its log with it, so it is `Ok(None)`. The
self-hosted route answers `200 []` for an id it has never heard of, so it
has **no 404 to give** — one means the address is not a mem0 server, and
reading it as `Ok(None)` is precisely the silent path above.

## A self-hosted listing has a ceiling and no cursor

Its listing route takes `top_k`, defaulting to **20** and capped at
**1000** — 1001 is a 422 — and it returns neither a cursor nor a total. So
a complete listing of a thousand memories and a truncated view of a hundred
thousand arrive byte-identically. `whole` refuses the count that sits
exactly on the ceiling for the same reason `list` refuses a listing cut
short by the budget: a partial listing handed back as a complete one reads
as every record past the ceiling having disappeared.

Leaving `top_k` unset would have been worse and quieter — twenty records,
looking complete.

## A scope that names nothing is not a wider listing of yours

The self-hosted route filters on `user_id`, `agent_id` and `run_id`, and
drops an `app_id` it does not know. A scope named only by `app_id`
therefore arrives naming nothing at all — and that route answers a request
naming nothing with **every memory in the store**, everybody's, as an
administrative listing. Both refusals happen at assembly, where a
misconfiguration should stop, rather than at the first read.

## This module has no way to declare anything, and that is now structural

mem0 has a metadata bag, and reading a `gmr` key out of it would be easy.
Doing so would advertise a declaration channel mem0 makes no promise about —
its update path says nothing about metadata surviving — and a channel that
works today and quietly stops tomorrow is worse than one that never existed.
Declarations for stores like this go through `gmr bind`, which is the base
primitive anyway.

The refusal used to be a value: every record came back carrying
`Claim::Silent`. It is now the absence of a trait — this module does not
implement `Declaring` and cannot, since that trait is synchronous and every
call here awaits. `Record` has no claim field for it to fill in. See
[[content-discovery]] for what the returned "I have none" cost, measured.

## Never writing is a property of the seam, not a rule anyone keeps

The `Http` trait this module talks through has `get` and nothing else, so
there is no method here that could write into somebody else's store. That
is why the guarantee needs no test: the alternative would not compile.

## What a fake can and cannot check

Everything decidable without a network — version derivation, history
reconstruction, both absence spellings, the exact routes each deployment is
asked for, the ceiling, budget exhaustion, the scope refusals — is tested
against `Canned`, and the self-hosted fixtures are the bodies a real server
actually returned rather than bodies read off its source.

What is left is whether mem0 still answers in that shape, and no fake can
know it. `tests/mem0_live.rs` is that canary: **one criterion set, aimed by
the environment at either deployment**, because the whole claim of this
module is that both are one store to GMR. It is a canary for API drift, not
a test of this crate's logic, and treating it as the latter would mean the
logic goes untested whenever no service is up.

The JSON structs here take `#[serde(default)]` on everything they can,
because this module reads a service it does not control: a field that
appears or disappears should not break a version derivation that never
looked at it. That is also why the two deployments share one set of
structs — they disagree about routes and status codes, not about what a
memory looks like.

## When this changes, ask

Does a write method appear on `Http`? That is the one guarantee this
battery exists to keep, and it is currently kept by there being nothing to
call.

Does anything start deriving a version from something other than the text —
a timestamp, an id, an ETag? Each of those reintroduces the failure this
hash was chosen to avoid, and the failure is silent: bindings read as
rewritten when nothing was rewritten.

Does a third deployment appear, or does a URL get built anywhere but in
`Deployment`? The reason every route, header and absence spelling is
decided in that one enum is that the previous arrangement — four string
literals across two files — is what let a whole dialect go unnoticed.

Does any dialect difference get expressed as a status code that both
deployments could return? `Absence` carries the probe URL so that the
deployment without a probe cannot ask for one. A `bool` in its place brings
back the reachable-but-wrong path.

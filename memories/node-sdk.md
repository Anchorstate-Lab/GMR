---
about:
  - domains/node/src/lib.rs#open
  - domains/node/src/lib.rs#Gmr
  - domains/node/src/lib.rs#Opening
  - domains/node/src/lib.rs#answered
  - tools/gate.py#check_typed_surface_names_the_contract
watch: [sig, logic]
---

# Seven verbs, one recipe entrance, and nothing that decides anything

`Runtime` has sixty-odd `pub async fn`. The binding exposes seven of them, plus
one way to say what a probe is:

```
sample(anchor, how)    read an anchor, and get the address of that reading
ground(claims, how)    do these sentences still stand. An address asks about
                       what the store holds; an object asks about a turn nobody
                       stored, and writes nothing
since(cursor, status)  what changed after this point in the journal
bind(claim, anchors, source, version, saw, asserts)
                       this sentence is about these anchors, and this is what
                       it was looking at when it said so
revoke(claim, source)  it is not any more
open(request)          open an anchor
close(key, why)        retire one, irreversibly
```

`sample` is the seventh, and it is the one that makes the other six worth
anything to a product that talks. Grounding answers whether the fact still
stands; it cannot answer whether the sentence was built from that fact at all,
and a caller that reads the world itself and binds afterwards is two readings
pretending to be one. `sample` hands the reading **and its address** to whoever
is composing the answer, `bind` takes that address back as `saw`, and `ground`
reports `shown` — see [[runtime-ground]]. One look at the world, cited.

It is not `observe` returning through a side door. `observe` writes and answers
what happened to the state; `sample` answers with the reading, which is what a
delivery path has to put in front of a model. `max_staleness` still decides
whether it goes and looks ([[runtime-instructions]]).

Deliberately absent, and not to be added back: `observe`/`look` (`max_staleness`
already says whether to go and look — see [[runtime-instructions]]), `pass`
(scheduling belongs to whichever process runs the loop, not to every caller),
`revise` and `accept --criteria` (**changing criteria is an owner's judgement**
and belongs in a reviewed commit, not in product code), and `health`/`links`/
`atlas` (operations and pictures, not a hot path).

This crate folds nothing, judges nothing, and retries nothing. It parses,
calls one runtime method, and serialises. Anything else it did would be a second
place where GMR's semantics live, in a language the tests are not written in.

## Why JSON crosses, and not a mirrored struct per contract type

napi can generate a TypeScript type from a `#[napi(object)]` struct, which is
tempting until you notice it means declaring `Standing`, `Grounding`, `Warrant`,
`Evidence`, `Anchored`, `Edges`, `Edge`, `Raised` a second time in Rust, with a
conversion each — the exact duplication [[transport-recipes]] had just deleted
on the other side of the crate. A second copy in the same language is a drift
path that nothing checks.

The contract types already serialise, and `contract::SHAPE` is an earned hash
over their declarations: change a field and `tools/gate.py` fails until somebody
records the new digest, and moving `CONTRACT` is what promises callers they may
match on it. So JSON crosses, and the guard that already exists guards it.

`dist/npm/index.d.ts` is that declaration, hand-written and pinned. Two checks
hold it honest and neither is enough alone: `check_contract_shape_is_earned`
refuses a contract type that changes shape while `CONTRACT` stands still, and
`check_typed_surface_names_the_contract` refuses a `.d.ts` naming a version the
runtime does not. So a shape cannot move without `CONTRACT` moving, and
`CONTRACT` cannot move without somebody editing this file — at which moment
they are looking at the declarations that have to move with it. The JS test
walks the discriminants (`grounding`, `holding`, `knowledge`, `anchored`)
against real output, because a rename passes both checks and breaks every
caller.

## Why input is deserialised rather than taken as a napi object

A `#[napi(object)]` silently ignores a property it does not know. `{ maxStaleness:
60000 }` would be dropped and the caller would get an answer served from the
record under a freshness bound they believe they set — invisible from outside,
indistinguishable from a fresh answer. Every input here is a `serde_json::Value`
deserialised with `deny_unknown_fields`, so the same typo is an error naming the
field it could not place. TypeScript would catch it at compile time for some
callers; the refusal catches it for all of them.

## What a caller may not hand in

`Expr` carries a `source` and the hash **earned** from it. Deserialised
field-for-field, a caller could supply a hash that does not match — and the
declaration hash is what every later reading is compared against. So an open
request writes rules as `{ when, to }` **strings**, and `authored` runs them
through `Expr::text`. The hash is computed on this side of the boundary or it is
not a hash of anything.

The same reasoning gives `close(key, why)` and `Supersede.rationale` text on the
wire and bytes inside: a rationale is prose, the sealer hashes bytes, and asking
JavaScript for an array of byte values would be asking it to do the encoding.

## `open` is a generic domain, so what a domain owns has to be passed in

`domains/coding` owns extractors, a coordinate syntax and `probes.toml`. This
domain owns none of that — its ontology arrives in the call:

```
open({ root, db?, recipes?, scripts?, providers?, policy? })
```

`root` is what `file`, `script` and `shell` probes are relative to; `db` defaults
to `<root>/.anchor/state/memory.db`, which is where the CLI keeps it, so an SDK
and a CLI pointed at one repository are [[store-journal-expected]]'s two writers
on one journal rather than two half-stories. `recipes` is
[[transport-recipes]] verbatim. `providers` names the memory stores to wire up.

Script-declared providers (`providers.toml`'s `[provider.<name>]`) are **not**
reachable from here: their declaration type lives in the coding CLI, and a
domain may not depend on another domain. Moving it into `batteries/provider`
is what that would take.

## When this changes, ask

Does an eighth verb arrive? Ask which of the four exclusions above it belongs
to, and if it belongs to none, why the seven were the seven. `sample` is the
precedent and it is a narrow one: it was added because there was a question
none of the six could answer, not because a caller found one of them awkward.

Does the binding start reading a field to decide something — a retry, a
threshold, a fallback? That is judgement, and the whole point of the boundary is
that judgement is on the caller's side of it.

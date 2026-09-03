---
about:
  - console/cli/src/verbs/bind.rs#run
  - console/cli/src/verbs/bind.rs#attest
  - console/cli/src/verbs/bind.rs#asserted
  - console/cli/src/verbs/bind.rs#assert_on
watch: [logic]
---

# The verb states a tie, and only tying needs the record to be there

Binding to anchors needs the record's current version, because the binding
states what the record said when it was tied there. `current_version`
supplies it — the same function `read` and `edges` use, so no second lookup
can drift from what those verbs report for the same reference.

A registered provider holding no such record answers `Ok(None)` and this
verb names it; a provider nobody registered is `Err(NoProvider)`. Neither
needs a per-store existence check: the generic answer is already the
specific one (see [[runtime-current-version]]). `root` is not in the
signature because nothing here needs it.

## `--detach` revokes, and never fetches

Detaching writes a revocation naming every live tag the record holds, one
per anchor, each recorded at that anchor. A revocation is not an assertion,
so it carries no version and asks the store nothing.

That is what lets it work in the state that most needs it: a record the
store reports as `gone` is exactly what `doctor` tells the owner to restore
*or detach*. A remedy a verb names and then declines to perform is worse
than no remedy named, because the reader stops believing the line.

Detaching a reference nothing is bound to is refused — there is no tie to
end.

## What it records about itself

A bind typed here is `Source::Adjudicated`: a person named the reference and
named the anchors. `sync` records `Source::Derived` for the same relation
reached another way. The two are told apart by kind of act, not by who ran
the command — see [[store-binding-record]].

## `attest` is a verb, not a flag on `bind`

`assert_on` is the whole act with none of the telling: take the version the
store can answer for, land the assertion, hand back what happened. `asserted`
prints this verb's version of that; [[cli-anchor-declares]]'s `--record`
prints its own. Two reporters, one place that decides what making a binding
costs — and one place a fourth caller would have to go through.

`attest` is where `Source::SelfAttested` is born: an agent wrote a record
into some store and is saying, itself, what that record is about. Both verbs
run `asserted`, which differs in nothing but the `Source` it hands down.

That one difference is why it is a separate door rather than `bind
--self-attested`. A flag has a default, and the default would be
`Adjudicated` — so an agent that forgot the flag would file its own say-so
as a person's judgement, which is the single reading `independent()` exists
to keep honest. A verb cannot be forgotten: you call it or you call `bind`.

Two consequences follow from what the act is, not from convenience:

- **`attest` only adds.** Ending a tie is a judgement about somebody's
  assertion, so `--detach` stays on `bind`.
- **An agent re-stamps a pending baseline by attesting again**, never by
  `reaffirm` (see [[runtime-reaffirm]]) — that verb records `Adjudicated`,
  and a second command must not launder the same agent's say-so into a
  second opinion. Attesting again is the same act again, and it carries a
  version whenever the store can answer by then.

A record too fresh for its store to answer for is the ordinary case here,
not the exception — it is the moment the link is most accurate — so
`asserted` binds with no version rather than refusing; see
[[runtime-standing-baseline]] for what a later reading does with that.

## The door says when a key was never opened here

`asserted` compares the landed anchors against the keys this store has opened
and names the ones it does not know — in prose and under `unopened` in `--json`.
It warns and still writes, because the record layer stays judgment-free: a
deployment may legitimately declare before it opens, and `attest`'s whole point
is running the moment the store hands an id back. What must not happen is
silence at the one moment the writer is present to hear it — a typo'd key
otherwise supervises nothing until a later `doctor` run finds it under
`unsupervised`, and `ground` answers `Ungrounded` about every invariant that
names it. The same door names the keys that have finished with nothing
succeeding them: a frozen journal will never observe the binding, which is
the `unopened` warning's twin — one key nothing ever watched, one key nothing
will ever watch again.

## A link nothing independent stands behind says so where it is read

`render.rs` marks a memory whose live assertions are all non-independent, so
the reader sees it at the same moment they read the memory. Two different
sentences, because the two are not the same claim: `Unknown` alone means
nobody recorded where the link came from, while a self-attestation means we
know exactly where it came from and it is the record's own writer.

Recording the source and then only ever showing it under `--json` would put
the fact in the store and out of sight of the person it is for.

## When this changes, ask

Does a new provider get its own existence pre-check bolted on here? The
provider's own answer through `current_version` is the generic "not here",
and it is specific enough that no store needs a special case.

Does a new flag start requiring the record to be fetchable? Ask what the
flag states. Anything that ends a tie can be stated without the store
answering at all, and requiring an answer means the flag stops working
precisely when the store has bad news — which is when it is wanted.

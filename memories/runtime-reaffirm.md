---
about:
  - crates/gmr-runtime/src/bind.rs#reaffirm
watch: [sig, logic]
---

# `reaffirm` exists to separate "I've seen new bytes" from "I mean something new"

`bind` takes `anchors` because that is where a caller states what a
reference is about. `reaffirm` does not take them at all: it re-stamps
`bound_version` over the union of the reference's live tags. That split
matters because the two situations are not the same event: content moving (a wording fix, a rebase) is "I've just seen
new bytes for something I already told you about," while changing
`anchors` is "I'm changing what this reference is about." If `reaffirm`
required `anchors` as an argument, every caller doing the first thing would
have to re-supply the second thing too, and a caller that got it slightly
wrong would silently rebind the reference to different anchors while
believing it was just refreshing a version stamp.

The union, not one row's copy: [[store-orset-projection]] can leave several
live assertions on a reference, and re-stamping one of them would drop the
rest.

Stating no aboutness is also why `reaffirm` writes through
`MemoryLens::bind` rather than `Runtime::bind`: it is not held to
[[runtime-bound]]'s idempotence guard. An assertion repeated says nothing
new, but a reading taken again at a later moment is a second reading, and
suppressing it would remove the only way to say "I have looked at this
again".

## What it records is a judgement, and the store can see that

`reaffirm` writes `Source::Adjudicated` — somebody looked at the rewritten
record and accepted it — which is what [[store-binding-record]]'s
`independent()` reads.

Nothing separates it from a hand-typed `gmr bind`, which records the same
word. Both are judgements, and no reader answers anything differently for
knowing which verb produced one. The difference that does exist —
reaffirming says "I read this version", binding says "this is what the
record is about" — is a claim about what a person attested to, and the
assertion layer records the *act*, not what was attested.

## When this changes, ask

Does the new code path let a version refresh also change `anchors` in the
same call, without the caller explicitly asking to rebind? That collapses
the distinction `reaffirm` exists to keep separate from `bind`.

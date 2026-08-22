---
about:
  - crates/gmr-runtime/src/bind.rs#reaffirm
watch: [sig, logic]
---

# `reaffirm` exists to separate "I've seen new bytes" from "I mean something new"

`bind` takes `anchors` because that is where a caller states what a
reference is about. `reaffirm` deliberately does not take `anchors` at
all — it looks up the existing `Binding` and re-stamps only
`bound_version`. That split matters because the two situations are not the
same event: content moving (a wording fix, a rebase) is "I've just seen
new bytes for something I already told you about," while changing
`anchors` is "I'm changing what this reference is about." If `reaffirm`
required `anchors` as an argument, every caller doing the first thing would
have to re-supply the second thing too, and a caller that got it slightly
wrong would silently rebind the reference to different anchors while
believing it was just refreshing a version stamp.

## What it records is a judgement, and the store can see that

`reaffirm` writes `Source::Adjudicated`: somebody looked at the rewritten
record and accepted it. That is the difference between a version stamp with
a person behind it and one a script refreshed, and it is what
[[store-binding-record]]'s `independent()` reads.

It does **not** yet separate `reaffirm` from a hand-typed `gmr bind`, which
also records `Adjudicated` — both are judgements, and no field distinguishes
which verb made them.

## When this changes, ask

Does the new code path let a version refresh also change `anchors` in the
same call, without the caller explicitly asking to rebind? That collapses
the distinction `reaffirm` exists to keep separate from `bind`.

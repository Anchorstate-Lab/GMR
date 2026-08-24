---
about: domains/coding/cli/src/verbs/sync.rs#align_bindings
watch: [logic]
---

# `align_bindings` writes only when the binding relation actually changed

`settled` is `had == want` plus `Bound::says` — the same predicate every
other writer is held to ([[runtime-bound]]), asked here as well so a note
that needs nothing produces no plan entry and no line in the report.

The two halves ask different things. `says` answers whether asserting would
move the projection; `had == want` is `align_bindings`' own extra question,
because a note that dropped a coordinate needs a revocation and `says` would
be satisfied without one.

A note whose live assertions do not yet include a `Derived` one is
unsettled, so `sync` derives it, once. The older assertions keep whatever
source they were recorded with — the table is append-only and an origin
that was never recorded really is unknown. Asking instead whether *every*
assertion behind the reference is `Derived` is a question about an immutable
past: it can never come back true, and every pass over an unchanged
repository re-asserts the whole corpus.

## A note declares its whole coordinate set, so dropping a line revokes

`about:` states everything the note is about, which makes `had - want` a
removal the note is asking for — `align_bindings` emits a revocation for it,
recorded as `Derived` like the assertion it undoes. Under
[[store-orset-projection]] an add can no longer take anything away, so
without this a line deleted from a note would leave its anchor bound
forever.

**Except when it looks like a rename.** Then the note is added to as usual,
no revocation is emitted, and the pair is reported for a person to judge —
so the new anchor is asserted without the old assertion being silently
dropped. Both anchors deliver in the meantime, which is the direction a
union should fail in.

That is one of two things this function decides. The other — whether a note
that dropped one key and gained another is a rename or a typo — lives in
`ambiguous`, and is written up in [[cli-sync-rename-ambiguity]]. Both run
before `rt.bind`, and they refuse for different reasons: `settled` declines
to write a row that would say nothing new, `ambiguous` declines to write a
row nobody has authorised.

## The reference is the source's, not one this function builds

The address a note is bound at is `note.reference` — the `Ref` the source
stamped on the record it handed over, carried through untouched. It is never
reassembled here from a provider constant and the note's path.

Reassembling is invisible while one store exists, because a constant and a
carried value produce the same bytes. With a second source the constant
keeps naming the first, so every record from the second is looked up in a
store that has never heard of it, and this function refuses to bind
anything while blaming a provider that is working fine.

The rule is wider than this call site: an answer already handed down is not
re-derived from a constant. The same applies in the subscription lookup
([[delivery-standing]]) and in how a note's name is spelled.

## It resolves; it does not write

`align_bindings` returns a plan and performs no writes. The journal is
append-only, so "atomic" cannot mean rollback — it can only mean *resolve
everything first, and write the first row only once nothing can still fail*.

That distinction was not academic. With writes interleaved, a sync that
failed while versioning the last note left every anchor before it open and
nothing bound: 346 anchors, zero memories. Both `check` and `doctor` read
that state as fine — an anchor with no memory yet is what `gmr anchor`
without `-m` produces on purpose, so neither verb has grounds to complain.
The half-finished state is invisible precisely because each half of it is
individually legitimate.

There is a test asserting the plan is not written, and a second assertion in
it applying the plan and finding the binding afterwards. The second looks
redundant and is not: without it the first would pass just as well against a
fixture that could never bind at all.

It takes the book of names for one reason: to name a note the way its author
does rather than by the address a store happens to keep it at. Leaving this one
printing paths would have meant two verbs spelling the same note differently,
which teaches a reader to trust neither.

It is handed one rather than building one, like every other verb — see
[[cli-main-run]]. Constructing a declaring source here would be a second opinion
about which sources exist, beside the one assembly already formed, and a second
opinion about a record's identity is the specific hazard this function sits
closest to.

## When this changes, ask

Does the new code call `rt.bind` unconditionally instead of comparing
against the current binding first? Every sync run would then add a row,
even when nothing about the note's anchors or version actually moved.

Does anything here start constructing a `Ref` rather than cloning the one
the note arrived with? Whatever it constructs it from is a second opinion
about where a record lives, and the source already gave the first.

Does a write move back into the resolving loop — a `bind`, an `open`, a
settings change? Every one of them turns a later failure into a repository
half-synced, and this repository cannot see that state.

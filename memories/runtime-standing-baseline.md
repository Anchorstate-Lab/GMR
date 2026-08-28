---
about:
  - crates/gmr-runtime/src/memory.rs#fetch_memory
  - crates/gmr-runtime/src/memory.rs#baseline
  - crates/gmr-runtime/src/memory.rs#dating
  - crates/gmr-runtime/src/memory.rs#says
watch: [sig, logic]
---

# The newest assertion is not the baseline; the newest one that took a reading is

A reference can hold several live assertions ([[store-orset-projection]]),
and since an assertion may be made while the store cannot answer for the
record, some of them carry no `bound_version` at all.

Two different questions are answered from that set, and `Bound` keeps them
apart so that every reader gets the same two answers ([[runtime-bound]]):

- **`standing`** — the newest assertion, whatever it says. It names the
  reference the view is about.
- **`baseline`** — the newest assertion that actually cited a version. It
  supplies `bound_version`, `bound_at_seq` and `baseline_at`, which is why
  those three can never name different rows.

Taking the newest assertion as the baseline throws a reading away. An agent
re-attesting a record its store has not indexed yet ([[cli-bind-run]]) is
saying "still about this anchor" and has compared nothing; reading its
silence as the new baseline turns a memory somebody verified into
`Unverified`, and the verification is not recoverable from the projection
afterwards. An assertion that verified nothing has nothing to overwrite a
verification with.

`Bound::baseline` is `None` when no assertion ever cited a version, and that
fallback to `standing` has a name — **`dating`** — because two callers need
the same answer to "which row is this binding dated by", and two copies of
`baseline().unwrap_or(standing)` are two chances to pick different rows.
That is the genuinely unverified case, and `baseline_at` stays `None` for it
rather than pointing at an assertion that established nothing.

## `says` has to weigh everything the row carries

`says` answers "would writing this row add nothing", and `bind` returns
without recording when it is true. It compared anchors, version and source —
everything a binding said *before* it carried a date. Once
[[runtime-bind]] started stamping `bound_at_seq`, re-stating the same claim
over an undated row did add something, and answering `this already stands`
made the omission permanent: a row written before the column existed could
never be re-dated, so [[runtime-warrant]] had nothing to compare it against
for the rest of its life. It was the answer for more than half of this
repository's own notes until `says` learned to ask `dating` whether the row
is dated at all.

Re-asserting is honest here and does not forge anything: `sync` re-reads a
note's own `about:` and binds as `Derived`, which is a fact about the file,
not a judgement about the code. The healing is also self-limiting — the new
row carries a seq, so the next run answers `already stands` and writes
nothing.

`sources` and `asserted_at` are still taken across the whole set, not from
either of these two: they describe how the link came to be, and every live
assertion took part in that.

## When this changes, ask

Does a new field get read off `standing` because it is "the current one"?
Anything about *what the record said* belongs to `baseline`; only what the
relation is about belongs to `standing`.

Does a binding row grow another field? Then `says` has to weigh it, or that
field silently never gets written for any binding that already stands.

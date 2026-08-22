---
about:
  - crates/gmr-runtime/src/memory.rs#fetch_memory
watch: [sig, logic]
---

# The newest assertion is not the baseline; the newest one that took a reading is

A reference can hold several live assertions ([[store-orset-projection]]),
and since an assertion may be made while the store cannot answer for the
record, some of them carry no `bound_version` at all.

Two different questions are answered from that set, and `fetch_memory`
keeps them apart:

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

`baseline` falls back to `standing` when no assertion ever cited a version.
That is the genuinely unverified case, and `baseline_at` stays `None` for it
rather than pointing at an assertion that established nothing.

`sources` and `asserted_at` are still taken across the whole set, not from
either of these two: they describe how the link came to be, and every live
assertion took part in that.

## When this changes, ask

Does a new field get read off `standing` because it is "the current one"?
Anything about *what the record said* belongs to `baseline`; only what the
relation is about belongs to `standing`.

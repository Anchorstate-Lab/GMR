---
about: crates/gmr-core/src/journal.rs#reason
---

# Derived, not stored alongside

`FailureCode::reason()` maps a code onto the class the substrate will actually act
on. It is **computed**, not kept next to the code — keep two copies and one day
some new code forgets to update the mapping, the two sides start disagreeing, and
nothing will ever notice.

This is the same discipline as `AnchorState.closed`: whatever can be derived from
facts already held does not get stored a second time.

## When this changes, ask

A need appears to "override the reason for some particular code" → that means the
codes are divided wrong. Split the code; do not open a back door in the mapping.

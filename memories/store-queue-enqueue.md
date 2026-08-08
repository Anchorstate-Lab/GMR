---
about:
  - crates/gmr-store/src/queue.rs#enqueue
  - crates/gmr-store/src/queue.rs#ensure_enqueued
watch: [sig, logic]
---

# `enqueue` resets state; `ensure_enqueued` never touches an existing row

These two exist side by side because they answer different questions.
`enqueue` is unconditional: it makes `anchor` due now and clears both its
lease and its parked state, even if the anchor already had a row —
callers reach for it when they specifically want to force a fresh cycle.
`ensure_enqueued` only inserts a row if one is absent; when it returns
`Ok(false)`, that means an existing row was found and left completely
untouched, backoff and park state included — a caller that only wants "at
least queued eventually" must not accidentally clear a backoff or park
that another part of the system set deliberately.

## When this changes, ask

Does a new caller actually want `enqueue`'s force-reset semantics, or does
it want "make sure this gets queued without disturbing anything already in
flight"? Reaching for the wrong one either fails to force a needed
refresh, or silently clears a backoff/park state someone else was relying
on.

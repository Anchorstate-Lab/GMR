---
about:
  - batteries/transport/src/inproc.rs#invoke
  - batteries/transport/src/inproc.rs#a_panic_is_recorded_not_propagated
watch: [sig, logic]
---

# A timed-out or panicking probe is our failure, recorded — never a crash

`invoke` runs the extract function on `spawn_blocking`, then races it
against `self.timeout`. Giving up on that race is all a caller can do: a
blocking thread cannot be cancelled from the outside, so on timeout it keeps
running to completion, unobserved. That is the price of not paying for a
real process boundary the way a subprocess transport would, and it is
called out here on purpose rather than left implicit.

A panic inside the extract function is caught through the `JoinError` from
`spawn_blocking` rather than allowed to unwind into the caller, because a
panic reaching the top would take the whole process down with it — and a
crash is not a journal entry. Recording it as `ProcessFailed` is what keeps
one bad probe from silently erasing everything else the run was going to
write down; without this, the whole CLI would die and nothing gets written.

## When this changes, ask

Does the new code path still convert a panic into a returned `ProbeError`
rather than letting it propagate past `invoke`? Any refactor that removes
the `spawn_blocking` + `JoinError` boundary reopens the "one probe's bug
kills the whole run" failure mode this exists to close.

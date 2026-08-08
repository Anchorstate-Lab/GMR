---
about: crates/gmr-store/src/journal.rs#Fence
watch: [sig, logic]
---

# `Fence` is an enum because "no token" and "token zero" must be refused differently

A lease's holder can still be working after the lease itself has expired —
that is exactly the situation the journal has to be able to refuse: a
write carrying a stale token. `Held(u64)` is the epoch one lease actually
issued; `Unleased` names a deployment that has no leases at all, where
there is no second writer to speak of and no epoch to compare against.

This is an enum rather than treating `0` as a sentinel for "no token"
specifically because an in-band sentinel would let a caller conflate "I
hold no token" with "my token happens to be epoch 0" — and `guard` (see
[[store-journal-guard]]) has to refuse those two situations for different
reasons, not the same one.

## When this changes, ask

Does a new variant, or a change to `epoch()`, reopen a way for "no token"
and "token 0" to look the same to a caller? `guard`'s two failure branches
depend on being able to tell them apart.

---
about:
  - crates/gmr-store/src/queue.rs#Queue
  - crates/gmr-store/src/queue.rs#lease
  - crates/gmr-store/src/sqlite/queue.rs#settle
  - crates/gmr-store/tests/conformance.rs#queue_contract
  - crates/gmr-runtime/src/observe.rs#observe_with
  - crates/gmr-runtime/tests/operations.rs#a_hand_run_observation_takes_the_lease_instead_of_slipping_past_it
watch: [sig, logic]
---

# The lease stopped being a correctness device and became an efficiency one

`lease` exists so a hand-triggered observation goes through the same
scheduling path as a due one — `observe_with` still threads a ticket's fence
into the append — and so two workers do not both fire the same
probe at the same target and burn two network calls for one answer. Not
getting it means somebody else is already on this anchor; the right response
is to let them do it.

**What it no longer supplies is correctness.** It used to be the only thing
standing between two writers and a lost update, which had two consequences
worth remembering: a deployment with no queue had no protection at all, and
the protection it did give was sized for crash recovery (`lease_secs`, tens
of seconds) when the thing actually needing exclusion was one `append`.
Two clocks in one number. The premise a write carries
([[store-journal-expected]]) is what keeps the invariant now, in every
deployment, queue or no queue.

## Fences must still only ever climb

`Queue` implementations still have to issue strictly increasing epochs per
anchor, and retiring must not reset the counter — `SqliteQueue::settle`'s
`Retire` arm parks the row (`parked = 1`) rather than deleting it, because a
fresh `INSERT` would restart from zero.

The reason is narrower than it was. Nothing refuses a write on this number
any more; it is hashed into the chain as provenance
([[store-journal-fence]]), and a counter that went backwards would make two
different lease generations indistinguishable in the record. That is a
weaker consequence than the one this invariant used to carry, and it is
still a reason to keep it.

## When this changes, ask

Could a new backend, or a retry or retire path, reissue an epoch that is not
strictly greater than every epoch issued before it for that anchor?

And separately: is anything starting to lean on the lease for correctness
again? A lease that is genuinely optional cannot be load-bearing — the
moment it is, a queue-less deployment is silently a different system.

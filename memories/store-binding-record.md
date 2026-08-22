---
about:
  - crates/gmr-store/src/bindings.rs#BindingRecord
  - crates/gmr-store/src/bindings.rs#Asserted
  - crates/gmr-core/src/memory.rs#Source
  - crates/gmr-core/src/memory.rs#only_a_record_that_declared_itself_or_was_judged_stands_on_its_own
watch: [sig]
---

# An assertion carries how it came to be, and only two of the ways stand on their own

`Asserted` is what a caller hands the store; `BindingRecord` is what comes
back. Both wrap a `Binding` with write-time metadata that is not part of the
relation — see [[memory-Binding]].

**A record is one assertion, not one reference.** `seq` names the row;
`binding.anchors` holds that assertion's still-live tags, so a caller can
revoke exactly what it saw. A reference with three assertions comes back as
three records; folding them into one view is the runtime's job, where
`MemoryView` lives. [[store-orset-projection]] defines "live".

## `Source` says how GMR learned the link

`Derived` — the record declared its own coordinate, in content that goes
through review. `SelfAttested` — the agent that wrote or used the record
asserted it. `Adjudicated` — someone reviewed and affirmed or revoked.
`Configured` — a provider recipe declared it. `Unknown` — nothing was
recorded.

It lives in `gmr-core` because the base holds the assertion and the base
answers `independent()`. Put in the domain, every domain re-derives what
counts as evidence — the one judgement a reader relies on this layer not to
invent.

**`independent()` is `Derived | Adjudicated`.** Self-attestation is the
agent vouching for itself: worth recording, since that is the most accurate
moment the link can be made, but not something a reader can weigh against
it. `Configured` is self-report with a longer life. `Unknown` is not
counted — claiming it would invent the fact being relied on. Under-crediting
is the safe error here; over-crediting is not.

## `bound_at_seq` is only meaningful with one anchor to have a head

`Option<Seq>`: "the bound anchor's head at bind time" has one unambiguous
answer only when the binding names exactly one anchor.

## The clock is the caller's

`Asserted` takes `at` rather than reading `Utc::now()` where the row is
written, the same way `Entry::Close` is handed its time, so a replay puts
back the moment the assertion was made.

## When this changes, ask

Does a caller assume `bound_at_seq` is `Some` for some anchor count other
than one? Inventing a `Seq` for a multi-anchor binding picks one anchor's
history as more important than the others.

Does a sixth `Source` arrive? Ask what it answers `independent()` with
first. Splitting by kind of act rather than by who acted is what keeps any
unverifiable identity out of the question.

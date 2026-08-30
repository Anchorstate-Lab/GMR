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
three records; folding them into one answer is the runtime's job, where
`Bound` lives ([[runtime-bound]]). [[store-orset-projection]] defines
"live".

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

## `bound_at_seq` dates the binding against the log

`Option<Seq>`: the journal's position when the assertion was made. `seq` is
global across anchors, so one number serves a binding that names any number
of them — which is the shape provenance actually has, one memory resting on
several facts.

`Option` survives for one reason only: rows written before the column
existed have no seq and never will, because this table is append-only.
Inventing one would date a binding to a moment nobody recorded.

## `saw` is what the asserter was looking at

A **set** of `FactAddress`: the readings the claim was made in front of. Not the
readings the anchors are on *now* — that is asked at read time — but the ones
whose facts the asserter had in hand when it said what it said.

A set and not one address, because an asserter reading four anchors looked at
four readings. It held one for a while, and the shape of the defect was this:
whichever anchor that address belonged to reported `seen` and every other one
reported `unseen` — a delivery-path failure invented by the record itself, on
every multi-anchor claim. Each anchor now asks whether **any** address in the set
is a reading it took, and a content hash from one anchor does not appear in
another's log unless they genuinely read the same thing.

Empty says the assertion cited none, which is what a note a person wrote does.
It is deliberately not the same as citing a reading nobody took: the runtime
reports those as `NotSaid` and `Unseen`, and only the second is a defect. See
[[runtime-ground]].

An assertion citing different readings is a different assertion, not a repeat, so
`Bound::says` compares the set — a rebind that changes only `saw` writes a row.

The column is one `TEXT` and holds both spellings: a bare 64-hex string for one
address, a JSON array for several. The table is append-only, so a migration that
rewrote the single-address rows was never available; reading both is not
politeness to old data, it is the only option there was.

## The clock is the caller's

`Asserted` takes `at` rather than reading `Utc::now()` where the row is
written, the same way `Entry::Close` is handed its time, so a replay puts
back the moment the assertion was made.

## When this changes, ask

Does a caller read `None` as "this binding is new"? It means the row
predates the column, which is the opposite — it is the oldest kind of row
in the table.

Does a sixth `Source` arrive? Ask what it answers `independent()` with
first. Splitting by kind of act rather than by who acted is what keeps any
unverifiable identity out of the question.

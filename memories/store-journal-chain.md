---
about:
  - crates/gmr-store/src/journal.rs#link
  - crates/gmr-store/src/journal.rs#Chained
  - crates/gmr-store/src/sqlite/journal.rs#appended
  - crates/gmr-store/src/sqlite/mod.rs#chain_the_journal
watch: [sig, logic]
---

# Append-only is enforced by a trigger; the chain is what answers when something gets past one

The journal already refuses `UPDATE` and `DELETE` by trigger
([[store-journal-guard]] covers the other half, stale fences). A trigger binds
whoever goes through this code. It binds nobody who opens the file with
`sqlite3`, and it says nothing at all about a copy that travelled.

Each row therefore carries `prev` — the hash it was linked onto — and `hash`,
over `{prev, anchor, fence, entry}`. Verifying is re-deriving every link and
finding the first row whose own hash no longer covers it.

## Over the canonical form, not over `body`'s bytes

`link` parses `body` back to a value and hashes the canonical encoding
([[addr-CanonicalizeError]]). Hashing the stored text would be cheaper and would tie
the chain to one backend's serialiser: the same entries exported and re-imported
come back as different bytes with identical meaning, and every one of them would
read as tampered. What the chain has to survive is a move between stores, which
is the same reason `Ref` and the fact address are canonical.

## No Merkle tree

A linked list gives tamper-evidence, which is the whole ask. A tree gives
*inclusion proofs* — "this entry is in the log you published" without shipping
the log — and nobody is publishing a log yet. The reason to lay the chain down
now anyway is that it cannot be added backwards later with any force: hashes
computed at time T commit to the state at time T, and every entry written before
the chain existed can only ever be blessed, never attested.

Which is exactly what this repository's own migration did to 58,534 rows, and
the honest reading of it: from v11 on, altering any of them breaks the chain;
about what they were before v11 it says nothing.

## This is why `append` takes the write lock first

Linking is a read of the tail followed by a write, and `pool.begin()` is
`BEGIN DEFERRED` — the read lock is taken first and upgraded at the write, which
under WAL meets `BUSY_SNAPSHOT` against a second writer, a class `busy_timeout`
does not retry. Two writers would also both read the same tail and link onto it,
forking the chain. `BEGIN IMMEDIATE` takes the write lock up front, so
contention becomes a wait. The migration ladder had been doing this since it was
written; the hot path had not, and a test with two writers on one file failed
with `database is locked` until it did.

## `Chained` is its own trait

Only some journals can answer this — the in-memory testkit has no file to
protect and nothing to attest to. GMR.md's rule is that a capability some stores
lack is expressed by not implementing a trait rather than by everyone returning
"I have none", the way `History` sits beside `ContentProvider`.

## Where it runs

`doctor`, every time, not `check`. It costs about a second on 58k entries
against `doctor`'s five, and `check` is the one that runs constantly. A tamper
check nobody runs is [[render-warrant]]'s classification nobody prints, so it is
not behind a flag.

## When this changes, ask

Does a second way to append appear? It has to link, and it has to take the write
lock first — a row with a `NULL` hash after v11 reads as *from before the chain*,
which is a claim the writer does not get to make quietly.

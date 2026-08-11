---
about:
  - batteries/survey/src/sqlite.rs#ready
  - batteries/survey/src/sqlite.rs#raise
  - batteries/survey/src/sqlite.rs#raze
  - batteries/survey/src/sqlite.rs#beneath
  - batteries/survey/src/sqlite.rs#strangers
  - batteries/survey/tests/durable.rs#an_index_this_build_cannot_read_is_rebuilt_rather_than_refused
  - batteries/survey/tests/durable.rs#a_database_the_index_did_not_write_is_refused_rather_than_razed
watch: [sig, logic]
---

# A derived store is razed and rebuilt, never migrated

The plan said the index would climb the same migration ladder the journal does.
It also said, four paragraphs later, that a probe version bump makes the old
data unreachable and so no migration code is needed. Only the second is right,
and the difference is not about SQLite — it is about what the two databases
hold.

The journal holds facts nobody can recompute: what was observed, when, and what
a person wrote about it. A journal from an unreadable generation has to be
refused, because getting it wrong destroys the only copy. That is why
[[store-migration-ladder]] exists and why every rung is proved against the
shipped schema.

The index holds nothing but a re-derivation of the repository. There is no copy
to lose. So its version check is: stamp matches, do nothing; stamp does not
match — in either direction — drop everything and rebuild. Migration code for
derived data buys avoiding one scan and pays for it with a class of bug that can
corrupt. `an_index_this_build_cannot_read_is_rebuilt_rather_than_refused` fixes
the asymmetry so nobody re-derives it from the journal's behaviour.

Note that this covers a **newer** index too. The journal refuses one, this
rebuilds it, and both are the same reasoning applied to different stakes.

## `raze` reads `sqlite_master` rather than naming the tables

The first version dropped the four tables it knew about. A test wrote a table
called `relic` into a stale database, and it survived — so every shape this code
ever stopped using would sit in the file forever, which is the unbounded growth
we set out to avoid, arriving as leftovers instead of generations.

It now enumerates `sqlite_master` and drops views, triggers, indexes and tables
in that order, skipping the `sqlite_%` internals nobody may drop. "Everything I
did not just create" is the only definition that stays true as the schema moves.

## Razing is only free on a file this code owns

Everything above is true of **an index**. `raze` ran on any database whose
`user_version` was not 1, and returned `Ok`.

Measured before the guard: a database holding one table `entry` with one row and
`PRAGMA user_version = 7` — the shape of the journal — came back from
`sqlite::open` as `Ok`, with its table gone and the four index tables in its
place.

The two are one filename apart. The journal is `.anchor/state/memory.db`, an
index sits beside it, both crates export a function called `sqlite::open`, and
both stamp the same `PRAGMA`. Nothing in the file said whose it was, so the
sentence at the top of this note — "there is no copy to lose" — was a claim
about the *index*, applied by the code to whatever it was pointed at.

`strangers` answers "is this mine": a database holding tables, none of them
`generation`, is refused with `Fault::Foreign`, naming what it found. An empty
file still builds; a file this code wrote at any stamp still razes, which is what
`an_index_this_build_cannot_read_is_rebuilt_rather_than_refused` keeps green.
That test is also what pins the marker: rename the `generation` table in `SCHEMA`
and the reopen in it stops recognising its own file and fails.

The lock-free stamp check deliberately stays **ahead** of the guard. A foreign
database whose `user_version` happens to be 1 is not razed either — it fails on
its first query instead, which is the loud side — and checking before the fast
path would cost a query on every open to catch a coincidence.

## The decision is inside the write lock

`ready` takes a lock-free read first, purely as the fast path: "already at this
version" needs no lock to be safe, because nothing follows from it. Every answer
that implies **writing** is taken again inside `BEGIN IMMEDIATE`, so a second
process arriving mid-rebuild reads what its neighbour just committed and finds
nothing to do.

This is the same shape as the journal's `rung`, for the same reason and learned
the same expensive way — deciding outside the lock is what made the journal's
first real upgrade fail with a bare SQLite error. `two_processes_opening_one_
stale_index_both_land` pins it here.

## The root predicate cannot be a prefix test

`beneath` compiles to `substr(f.rel, 1, n) = root AND substr(f.rel, n + 1, 1) =
'/'`, not `LIKE root || '%'`. `crates/gmr-core` must not draw in
`crates/gmr-core-extra`, and `LIKE` would also have to escape `%` and `_` out of
the root itself.

The conformance fixture carries `b.rs`, `b/x.rs` and `bb.rs` for exactly this:
under the root `b`, only `b/x.rs` qualifies, and both of the others begin with
the same letter. Checked by replacing the second `substr` with a tautology — the
suite fails there and nowhere else, and the in-memory arm stays green, which is
what tells you the disagreement is the backend's.

The ordering trap has the same shape of guard: swapping `ORDER BY f.sort` for
`ORDER BY f.rel` fails only the SQLite arm, on `b/x.rs` against `b.rs`. See
[[survey-index-shape]] for why the sort key is handed in rather than derived.

## When this changes, ask

Has anyone given the index a ladder? If a rung ever looks necessary, the
question to answer first is what the index knows that the repository does not —
if the answer is nothing, the rung is a way to corrupt data that a rebuild
recreates for free.

---
about:
  - crates/gmr-store/src/sqlite/mod.rs#migrate
  - crates/gmr-store/src/sqlite/mod.rs#climb
  - crates/gmr-store/src/sqlite/mod.rs#rung
  - crates/gmr-store/src/sqlite/mod.rs#under_the_write_lock
  - crates/gmr-store/src/sqlite/mod.rs#a_climbed_database_ends_up_shaped_like_a_freshly_built_one
  - crates/gmr-store/src/sqlite/mod.rs#a_whole_v6_database_climbs_into_the_shape_a_fresh_build_makes
  - crates/gmr-store/src/sqlite/mod.rs#blueprint
  - crates/gmr-store/src/sqlite/mod.rs#two_openers_racing_the_same_upgrade_do_not_both_apply_it
watch: [sig, logic]
---

# A database from the past is carried across; only one from the future is refused

`migrate` used to refuse any database whose stamp was not exactly this build's
version. That is a safe default and a bad one: it turns "add a nullable column"
into "every user exports on the old binary and imports on the new one". The
asymmetry it was missing is that the two directions are not alike.

```
stamped == 0            nothing here yet   ->  build the whole schema, stamp it
stamped <  this build   an older shape     ->  climb, one rung at a time
stamped == this build   nothing to do
stamped >  this build   written by a later generation, whose shape we cannot know
                                           ->  refuse. Reading it wrong is worse
                                              than not reading it
```

Only the last case keeps the old refusal, and it now says to upgrade rather than
implying the database is broken.

## Why the schema is not re-applied after a climb

`SCHEMA` is all `CREATE ... IF NOT EXISTS`, so running it after climbing would be
harmless — and that is exactly the problem. It would paper over a rung that
forgot an index or a trigger, and
`a_climbed_database_ends_up_shaped_like_a_freshly_built_one` would pass while the
two paths had silently diverged. A ladder has to be sufficient on its own, so it
is never given a safety net that hides when it is not.

That test compares `sqlite_master` between a database built from scratch and one
carried up by the ladder. `that_comparison_can_actually_fail` runs the same
comparison against a rung that drops the index, and asserts the two disagree —
without it, the first test proves only that it was written.

### The mechanism is proved on a toy schema; the shipped rung needs its own

Those two run on a three-object fixture, which is right for proving the
comparison works and wrong for proving *this* ladder works. So
`a_whole_v6_database_climbs_into_the_shape_a_fresh_build_makes` runs it on the
real pair: a frozen, complete v6 schema — every table, index and trigger a v6
database in the wild actually has — climbed to v7 and compared against what this
build makes from scratch.

It cannot reuse `shape`. That helper compares `sqlite_master.sql` **verbatim**,
which is exact and cheap while both paths execute the same DDL text, and simply
untrue here: `ALTER TABLE ... ADD COLUMN` makes SQLite rewrite the stored
`CREATE TABLE` text by appending to it, so the climbed `settings` carries the v6
wording plus a tail while the fresh one carries the v7 wording. Two identical
shapes, two different strings. `blueprint` therefore compares tables through
`pragma_table_info` — name, type, nullability, default, primary key — and
non-tables through their whitespace-squeezed SQL, so formatting cannot fail it
and a forgotten column, index or trigger cannot pass it. Both halves were
checked red: retyping the rung's column to `TEXT` fails it, and deleting one
trigger from the v6 fixture fails it.

The v6 fixture is frozen on purpose. Deriving it from `SCHEMA` by stripping the
new column would make it track whatever the schema becomes, and the comparison
would then be a mirror agreeing with itself.

### It also catches the mistake nobody plans to make

Editing `SCHEMA` and forgetting to move `SCHEMA_VERSION` is the way this rots in
practice, and it is worse than it looks: fresh installs get the new object and
every database already in the field does not, because nothing re-applies
`SCHEMA` after a climb. Two populations, one version stamp, silently different.

No gate has to look for that, because this test already fails on it. The frozen
fixture climbs through the rungs that exist; a `SCHEMA` that grew a column the
rungs do not know about produces a fresh build the climbed database cannot
match. Checked by adding a column and running it: red on
`a_whole_v6_database_climbs_into_the_shape_a_fresh_build_makes`, and on nothing
else.

The one thing it does not see is `SCHEMA`'s pragmas — `journal_mode`,
`foreign_keys` — which are not objects in `sqlite_master` and are not compared.
Those are settings on the connection, not shape a database carries, so changing
one is not a migration; if that ever stops being true, this is the sentence that
was wrong.

## The version has to be read inside the write lock, not before it

Deciding what to do and doing it are one step, not two. The first version read
`PRAGMA user_version` outside any transaction and then opened a **deferred**
one, so two processes starting together both saw v6 and both tried to apply the
rung. A rung is an `ALTER TABLE`, which is not idempotent: one of them lost with
a bare SQLite error — `duplicate column name`, or `database is locked` depending
on how the two interleaved.

Nothing was corrupted and a retry succeeded. What was lost is worse than that
sounds: **the whole vocabulary above was bypassed on the only run that ever
reaches this path.** `SchemaVersionMismatch` and its "export it with the version
that wrote it" sentence exist to tell a person what to do, and a first upgrade
under any concurrency handed them a SQLite internal instead.

`rung` now takes `BEGIN IMMEDIATE` — SQLite's write lock, acquired at BEGIN
rather than on first write — and re-reads the stamp **inside** it. A process
that arrives second reads the version its neighbour just wrote and lands
without doing anything. The loop is per rung rather than one transaction around
the whole ladder, so the resumability below survives.

`migrate` still reads the stamp unlocked first, purely as a fast path: the
overwhelmingly common answer is "already at this version", and that answer needs
no lock to be safe. Every answer that implies *writing* is taken again under the
lock, where it is authoritative.

## Why the stamp is inside the rung's transaction

Each rung applies its SQL and moves `PRAGMA user_version` in one transaction, and
SQLite makes `user_version` part of that transaction. So a rung either lands
completely and is stamped, or does not land and is not — a failed migration
leaves the database at the last version that fully succeeded, and the next run
resumes from exactly there instead of replaying work or skipping it.

## The first rung, and what it cost

v6 → v7 adds `settings.budget_ms`, one nullable column. The whole rung is one
`ALTER TABLE`, which is the point: the mechanism landed one commit before the
first migration needed it, so the migration itself was a line of SQL rather than
a line of SQL plus a mechanism plus an argument about the mechanism.

It was exercised on a real database, not only a fixture: this repository's own
journal, 12.7 MB and 9867 entries stamped v6, opened under the new build, came
back stamped v7 with all 235 settings rows intact and the new column reading
NULL — no opinion, which is exactly what a setting nobody has expressed should
say. Under the old refusal that database could only have been moved by exporting
it on the old binary and importing on the new one.

`the_ladder_has_a_rung_for_every_version_it_claims_to_cross` guards the gap that
appears from here on: adding a version without adding its rung makes every
database at the version below permanently unopenable, and nothing else would
notice until a user hit it.

## When this changes, ask

Does raising `SCHEMA_VERSION` come with a rung from the version below it, and
does that rung produce the same `sqlite_master` as building the new schema from
scratch? Those are the two ways this rots, and only the second one is subtle:
the ladder and the full schema are two descriptions of one shape, and nothing
except that comparison keeps them saying the same thing.

---
about:
  - crates/gmr-store/src/sqlite/mod.rs#migrate
  - crates/gmr-store/src/sqlite/mod.rs#climb
  - crates/gmr-store/src/sqlite/mod.rs#a_climbed_database_ends_up_shaped_like_a_freshly_built_one
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

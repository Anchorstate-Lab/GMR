---
about:
  - batteries/transport/src/sql.rs#Ask
  - batteries/transport/src/sql.rs#Source
  - batteries/transport/src/sql.rs#version
  - batteries/transport/src/sql.rs#cell
  - batteries/transport/src/sql.rs#shaped
watch: [sig, logic]
---

# A probe observes, and this is the family that could have written

M2's third and last. Same shape as [[transport-http]] and [[transport-file]] — a
place and a way to pick one thing out of it — with the bytes coming from a
database. The declaration is a connection reference, a query, and optionally which
column to take.

## The connection is opened read-only, and that is not a convention

A connection url is the first thing here that can *change* the world it reports
on. `UPDATE` in a declaration would be reviewed like anything else and would still
be one careless review away from a probe whose reading moves the fact — which
makes every anchor downstream of it meaningless, silently, in a direction nothing
detects.

So `read_only(true)` and `create_if_missing(false)` are set on the connection and
the **driver** refuses the write. A test issues an `UPDATE`, asserts it is
refused, and then asserts the row is still what it was. Hoping declarations only
ever `SELECT` is not the same thing.

## What is earned, and the one thing that must never be

The version covers the query, the column, and **which reference** the connection
comes from — `Given(url)` hashes the url, `FromEnv(var)` hashes the variable's
name and never reads it.

A connection url carries a password. Hashed by value, rotating a database
credential would report every anchor behind that database as read by a different
instrument, and the whole set would go `Incomparable` on the day somebody did the
responsible thing. A test rotates the secret and asserts the version does not move.

The timeout stays out for D-11's reason: it decides whether there is an answer,
not what the answer is.

## Three answers, and a missing database is not the same as a missing file

```
0 rows            -> Outcome::NotFound      the database answered: no such fact
1 row             -> the value
2+ rows           -> Unusable               the declaration means more than one thing
NULL column       -> NotFound               the row holds nothing there
query refused     -> Unusable               the schema is not what the declaration assumed
cannot connect    -> Unreachable            nothing about the row was established
```

That last line is where this family deliberately differs from [[transport-file]].
A config file that is not there **settles** what the config says. A database that
is not there settles nothing about the row — we never got to ask. Reporting
absence there is the OCSP mistake constraint 4 names, so it stays `Unreachable`.

## Several columns come back whole

With no `column` named and more than one returned, the row is reported as an
object rather than by picking the first. Picking by position would change what an
anchor watches the day somebody reorders a `SELECT`, and nothing would say so.
Naming a column the query does not return is `Unusable`, and the error lists what
it does return.

`cell` reads INTEGER as a number and TEXT as a string, because state comparison is
by value: a number that arrives as `"1700000000"` never equals one that arrives as
`1700000000`, and an anchor would report a change every time the driver's mood
shifted. BLOBs come back `Null` — bytes are not a fact anyone can read in a diff.

## Why the path is not confined the way `file`'s is

[[transport-file]] refuses any path leaving the tree. This does not, and the
difference is where the constraint is free. Config files live in the repository;
databases legitimately do not. What is reviewed here is the **query**, which is the
artifact that decides what comes back — and it cannot read an arbitrary file the
way an unconstrained path can.

## When this changes, ask

Does `read_only` come off for any reason? Then a probe can move what it reports,
and no anchor downstream of it means anything.

Does a missing database start reading as `NotFound`? That is absence claimed by
somebody who never got to look.

Does a second backend arrive? The url is already a reference, so the credential
story holds — but check that its driver has a read-only mode that is actually
enforced, rather than a flag it accepts and ignores.

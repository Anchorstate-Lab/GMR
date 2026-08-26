---
about:
  - batteries/transport/src/sql.rs#Ask
  - batteries/transport/src/sql.rs#Source
  - batteries/transport/src/sql.rs#version
  - batteries/transport/src/sql.rs#cell
  - batteries/transport/src/sql.rs#shaped
  - batteries/transport/src/sql.rs#sqlite_url
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

## A backend this build cannot speak is a declaration to fix

`SqliteConnectOptions::from_str` accepts **any** string and treats it as a
filename, so `postgres://user:pw@host/db` becomes a file by that name and failing
to open it came back `Unreachable` — a transient outage an anchor backs off and
retries forever, for something that can never work. `sqlite_url` checks the scheme
first and refuses anything else as a declaration to fix.

The audit that found it also checked the thing that would have been worse: sqlx's
connect error does **not** quote the url back, so a password in a connection
string does not reach the journal through that path. The refusal above names only
the scheme.

## A local database is not a remote system

`resolve` reports `Closed` for a sqlite url and `Open{Network, Clock}` otherwise.
It claimed `Network` unconditionally at first, which was simply false for a local
file — and the opposite call from [[transport-file]], which reads a local file and
is `Closed`, and from the extractors, which do the same. Over-claiming openness is
not the safe direction; it is a wrong answer a reader has no way to check.

A `FromEnv` source is treated as remote, because what it resolves to is not known
while deciding what the instrument is, and guessing optimistically there is the
wrong way to be wrong.

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

The reading is capped against `budget.output_cap()` before it is returned — a
query is the easiest of the three families to point at something enormous, and
storing a truncated reading as fact would be a lie. That check was missing until
an audit put the three families side by side: each had exactly one half of
`Budget` and it was a different half each time, which is what copying a shape
without a checklist looks like.

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

---
about:
  - batteries/transport/src/sql.rs#Ask
  - batteries/transport/src/sql.rs#Source
  - batteries/transport/src/sql.rs#tellable
  - batteries/transport/src/sql.rs#version
  - batteries/transport/src/sql.rs#cell
  - batteries/transport/src/sql.rs#shaped
  - batteries/transport/src/sql.rs#spoken
  - batteries/transport/src/sql.rs#postgres
  - batteries/transport/src/sql.rs#Kept
  - batteries/transport/src/sql.rs#told
  - batteries/transport/src/sql.rs#sqlite_outage
  - batteries/transport/src/sql.rs#endpoint
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

The version covers the query, the column, which fields the query **binds** from
the position (see [[transport-template]]), and **which reference** the connection
comes from — `Given(url)` hashes the url, `FromEnv(var)` hashes the variable's
name and never reads it.

A connection url carries a password. Hashed by value, rotating a database
credential would report every anchor behind that database as read by a different
instrument, and the whole set would go `Incomparable` on the day somebody did the
responsible thing. A test rotates the secret and asserts the version does not move.

The timeout stays out for D-11's reason: it decides whether there is an answer,
not what the answer is.

## Which backend a url names is one decision, made in one place

`SqliteConnectOptions::from_str` accepts **any** string and treats it as a
filename, so `postgres://host/db` becomes a file by that name and failing to open
it came back `Unreachable` — a transient outage an anchor backs off and retries
forever, for something that can never work. The scheme is read first, and a url
nothing here speaks is refused as a declaration to fix.

That check used to be `sqlite_url(&str) -> bool`, and a boolean can only ever
answer "sqlite or not": adding postgres to it would have meant a second boolean
beside the first, and a third backend a third — which is the accumulation the
rules forbid. `spoken` returns **which** dialect, `None` for one nothing knows,
and `invoke` branches on it. G3 added a backend by adding an arm, not by
routing around the decision.

`Spoken::local` is what decides `Verifiability`: sqlite is a file and reads
`Closed`, postgres is across a network and reads `Open{Network, Clock}`. A second
backend does not get to be quieter about that than the first.

The audit that found it also checked the thing that would have been worse: sqlx's
**sqlite** connect error does not quote the url back, so a password in a
connection string does not reach the journal through that path today. The refusal
above names only the scheme.

"Today" and "sqlite" are both load-bearing, which is why G1.5 stopped relying on
them. `Source::tellable` passes the driver's own words through only when the url
was `Given` — reviewed, and refused outright if it carries userinfo — and for a
`FromEnv` url says the variable's name and that the reason is not being repeated.
The next driver to arrive is not audited yet and does not have to be: see
[[transport-given]].

## A local database is not a remote system

`resolve` reports `Closed` for a sqlite url and `Open{Network, Clock}` otherwise.
It claimed `Network` unconditionally at first, which was simply false for a local
file — and the opposite call from [[transport-file]], which reads a local file and
is `Closed`, and from the extractors, which do the same. Over-claiming openness is
not the safe direction; it is a wrong answer a reader has no way to check.

A `FromEnv` source is treated as remote, because what it resolves to is not known
while deciding what the instrument is, and guessing optimistically there is the
wrong way to be wrong.

## The endpoint is kept, and the credential never becomes a key

Every `invoke` used to build a pool, run one query, and close it. Against a local
postgres in Docker that is a 42 ms handshake for a 2.5 ms query — measured
through the transport at **579 ms for the first reading and 24 ms for every one
after**, once the pool is kept.

`Kept` holds at most eight, evicting the least recently used, with an idle
timeout and a max lifetime so a pool whose credential has been rotated away
drains rather than holding connections open on a revoked one forever.

The key is the **hash** of the resolved url, never the url. A rotated credential
hashes differently and gets its own pool, which is the correctness the cache
needs; and the thing sitting in a long-lived map is not a password. That is the
same rule as [[transport-given]] one layer in: a credential is held by reference,
never by value, and never anywhere it could be printed.

## Which side a failure blames is decided by the error, not by where it was raised

Connect meant an outage and query meant a bad declaration, and that was only ever
right because every reading reconnected. With the pool kept, a database that goes
away surfaces **inside the query** — and reading that as `Unusable` is the OCSP
mistake constraint 4 names, an outage reported as a declaration to fix.

So the class comes from the error. A non-database error is always an outage. A
database error is a refusal **unless its code says otherwise**, and which codes
those are is each backend's own vocabulary:

```
sqlite     primary code 14 CANTOPEN · 10 IOERR · 11 CORRUPT
postgres   SQLSTATE 08* connection exception · 53* insufficient resources · 57P03
```

The sqlite list is not defensive. A test deletes the file between two readings
and asserts the second is not `Unusable` — sqlite reports "unable to open
database file" as a database error with code 14, so kind alone gets it wrong, and
this was found by writing that test rather than by reasoning about it.

`acquire_timeout` carries the budget, and there is deliberately no
`tokio::time::timeout` wrapped around the connect. One was added and had to come
out: with both set to the same span the outer one wins the race, sqlx never gets
to return `PoolTimedOut`, and a port nothing is listening on comes back as a
spent budget instead of an outage. The bound belongs to the layer that can also
name the reason.

It is taken from the call that **creates** the pool, and later readings acquire
under whatever that first call allowed. That is the wart the cache buys with, and
it is bounded: acquiring from a live pool does not block.

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

## Postgres: read-only is the server's word, and the credential is not in the file

sqlite gets `read_only(true)` from the driver and postgres has no such flag, so
the session says `SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY` on
connect and **the server** refuses a write. That is the same kind of enforcement
and not a promise this crate makes to itself; a test drives an `UPDATE` through a
real postgres and asserts both the refusal and that the row did not move. It is
said as a statement rather than passed as `options=` in the startup packet
because a pooler in front of the database may reject the latter and would have to
be trusted to relay it.

**A postgres url in a declaration may not carry userinfo at all** — not even a
bare username, see [[transport-given]]. That looks strict until you notice what
it buys: a coordinate typed at a terminal cannot say `from_env`, and
`gmr anchor 'sql://postgres://host:5432/db#SELECT …'` still works in one step,
because `PgConnectOptions` starts from `PGUSER`/`PGPASSWORD` and the url only
overrides what it names. So the file carries host, port and database — none of
them a secret — and postgres's own environment carries the rest. That is its
answer to this question, not a syntax invented here.

A column type this build has no reading for is an **error naming the column and
asking for a cast**, not a `Null`. sqlite returns `Null` for a BLOB, which is
defensible for one type nobody anchors; postgres has dozens, and quietly
reporting `null` for a `timestamptz` would be this transport saying something the
database did not.

## Why the path is not confined the way `file`'s is

[[transport-file]] refuses any path leaving the tree. This does not, and the
difference is where the constraint is free. Config files live in the repository;
databases legitimately do not. What is reviewed here is the **query**, which is the
artifact that decides what comes back — and it cannot read an arbitrary file the
way an unconstrained path can.

## A query that recomputes what the product computes is two facts, not one

The declaration decides what comes back, and that is exactly what makes this
family the easiest place in GMR to install a second copy of somebody else's
business logic. A read-only `SELECT` that rebuilds "the price a guest is quoted"
out of the tables the product stores is not observing the product: it is a
second implementation of it, running beside the thing it claims to watch.

It happened, in a product built on this crate. The query reassembled a menu
price from `product` and `price_promotion`; the application's own Python also
checked the promotion's days of week, its local start and end times, and which
channels it applied to. The query did not. The product told a guest 4.20 and the
anchor held 3.36, both entirely self-consistent, for as long as anybody cared to
look — and every claim resting on the anchor still came back `Holds`, because
each side was right about its own computation and neither could see the other.

**Read what the product has already computed and stored.** Its own output
endpoint, its own materialised column, its own view. If the only way to observe
a fact is to recompute it, that is worth saying out loud before writing the
query, because the anchor is then watching an implementation nobody deploys.

`Unseen` ([[runtime-ground]]) is what catches this now, and only because the
answer and the anchor are made to share one reading. It does not make a
recomputing probe correct; it makes it visible.

## When this changes, ask

Does `read_only` come off for any reason? Then a probe can move what it reports,
and no anchor downstream of it means anything.

Does a missing database start reading as `NotFound`? That is absence claimed by
somebody who never got to look.

Does a **third** backend arrive? Answer the same two questions postgres had to.
Does its driver have a read-only mode that is really enforced rather than a flag
it accepts and ignores? And does its url form let a credential be left out?

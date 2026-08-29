---
about:
  - batteries/transport/src/template.rs#url
  - batteries/transport/src/template.rs#path
  - batteries/transport/src/template.rs#held
  - batteries/transport/src/sql.rs#bound
watch: [sig, logic]
---

# A declaration says what the probe is; the position says where it is pointed

`ProbeCall` has carried a `position` since the beginning, and until G1.5 the
http, file and sql transports each ignored it completely. `invoke` went straight
to `asks.ask(name)` and used whatever url, path or query it found there. So one
declaration meant exactly one place, and watching `replicas` in staging and in
production meant two declarations with the same selector and two urls in them,
free to drift apart with nothing to notice.

That is the same mistake in the small that [[transport-position-env]] avoided in
the large: the shell transport has always handed the position to the probe. These
three had a channel and did not read it.

## The version is earned from the template, never from an expansion

This is the constraint everything else follows from. `Ask::version` hashes
`envs/{env}.yaml`, not `envs/prod.yaml`. Were it the other way, every anchor on
one endpoint would have been read by a different instrument from every other, and
no two of their observations could be compared — `Incomparable` across a set of
facts that are, by construction, the same measurement.

It is also why `file`'s `reading()` stays on the template: which parser runs is
part of what the instrument *is*, so `shaped` is a thing the declaration decides
and a position may not change.

## What each family already had, used instead of a fourth invention

- **http** — URI templates, **RFC 6570 level 1 only**: `{name}`, simple
  expansion, percent-encoded against the unreserved set. Not `{+name}`, not the
  operators; a subset is honest and additive, and claiming the whole RFC while
  implementing a corner of it is not.
- **file** — the same scan with no escaping, because a file path is not a URI and
  percent-encoding a filename produces a different filename. Containment is
  unchanged and is `inside()`'s, which now runs on the **assembled** path rather
  than the declared one — that distinction did not exist while templates did not.
- **sql** — no templates at all. **Bind parameters**, which is what a database
  offers for exactly this, and the reason the injection test in `sql.rs` passes
  without this file being involved. `binds` names the position's fields in
  parameter order rather than scanning the query for `:name`: SQL-aware scanning
  has to get string literals and `::` casts right, and a list a person wrote
  cannot be wrong about what it meant.

`binds` is hashed with the query, because a query that reads a parameter from the
position is a different instrument from one that does not. What the field
*holds* is not, because that is the position.

## A name the position cannot fill is ours, not the world's

`ArtifactInvalid`, and nothing is asked. `NotFound` would say the endpoint
answered and the thing is not there; no request went out at all. The message
names both halves — the template's name and the position it was handed — because
this failure is always a declaration and an anchor disagreeing about what is
being watched, and a reader needs to see both to know which one is wrong.

A list or an object where a name is expected is refused for the same reason:
a position names one place, several is not one, and picking one here would be
this transport inventing which.

## When this changes, ask

Does an expansion start being hashed — for a cache key, for a nicer `probes list`
line, to make two positions distinguishable? Then one instrument becomes many and
the corpus goes `Incomparable` at itself.

Does `sql` grow a template? A query built by substitution is an injection whether
or not the value looks dangerous today, and the database already offers the thing
that cannot be one.

Does `file` start percent-encoding, or `http` stop? They look like the same
operation and are not: one produces a URI, the other produces a filename.

---
about:
  - batteries/transport/src/given.rs#without_credentials
watch: [sig, logic]
---

# A password written into a declaration is a password in the journal, and the journal does not forget

`ProbeError.message` is written to the journal **verbatim**, inside an
`Entry::Attempt`, on an append-only hash chain. There is no edit and no delete: a
secret that reaches this string is a secret that is in every copy of the
repository from then on, and the only remedy left is to rotate the credential.

[[transport-http]] already held that line for header values. It did not hold it
for the url, and a url is where credentials most often hide —
`postgres://svc:hunter2@db/app` is the ordinary spelling of a DSN, and
`https://user:token@host/x` is a real thing people paste. Two doors were open at
once:

- **We put it there ourselves.** Every http status branch interpolated `ask.url`,
  and `reqwest::Error`'s own `Display` appends the url it was given. sqlx's
  sqlite driver happens not to — which was audited once, holds for one driver,
  and is not a property to keep depending on.
- **It was allowed to be written down.** `Source::Given` and `Ask.url` accepted
  whatever a declaration said, and `gmr anchor 'https://u:p@host/x#$.y'` wrote it
  into `.anchor/probes.toml` — a file people commit.

## Both doors, not one

The messages now name **the probe**, never the endpoint. `the endpoint 'quote'
reads answered 500` says everything a reader needs — which of their probes, and
what happened — and says nothing to anyone who should not have it. `reqwest`'s
error goes through `without_url()`; sqlx's goes through `Source::tellable`, which
passes the driver's words on only when the url was `Given` — reviewed, and
refused outright if it carries userinfo — and otherwise says the variable's name
and that the reason is not being repeated. That one holds for a driver nobody has
audited, which is the point: G3's postgres branch does not need a second audit. `ProbeError::about` puts the probe's name on errors
raised deeper down, where the name is not in scope.

And a url with anything before an `@` in its authority is **refused** — at
`invoke`, and again in `gmr anchor` before the declaration is written, because a
refusal that happens after the write leaves the password in a file the person is
about to push. The refusal says what to do instead: name an environment variable,
and the value is read at the moment of the call and never stored. That is the same
rule [[transport-recipes]] states for recipes travelling as data, arriving at the
same place from the other side.

A sqlite file whose *name* contains an `@` is refused too. That is a real false
positive and it is the right trade: the check knows no schemes, so it cannot be
wrong about which ones carry credentials, and the message tells whoever hits it
exactly what it saw.

## When this changes, ask

Does an error start quoting the url again — for a clearer message, for a
diagnostic, because the probe name felt too thin? The journal keeps whatever
reaches it, and no later commit takes it back out.

Does the check start being scheme-aware, so `sqlite://` is exempt? Then the one
rule becomes a table of exceptions, and the first backend nobody thought about
gets in for free.

---
about:
  - batteries/transport/src/http.rs#Ask
  - batteries/transport/src/http.rs#Header
  - batteries/transport/src/http.rs#version
  - batteries/transport/src/http.rs#sent
  - batteries/transport/src/http.rs#pointer
watch: [sig, logic]
---

# The first fact that is not a file path, and the two lines it had to draw

M2's opening move: a probe whose subject is a URL. It is a feature and a module,
not a package — the crate's own manifest said so before this existed, and
[[transport-script]] is the shape it follows: identity computed at call time from
the declaration, not asserted by the thing being measured.

## What the version is earned from, and what it is deliberately blind to

`Ask::version` hashes the url — the **template**, since G1.5, never an expansion
of it; see [[transport-template]] — the selector, and each header's **name plus
where its value comes from**. Rule 5 wants everything that can change the output; the
selector decides what the output *is*, so it is in.

Two things are out, and both are D-11's line:

- **The timeout.** It does not change the output, it changes whether there is
  one. A budget that runs out shows up as `Blind::NeverAsked` on the knowledge
  axis; `ProbeVersion` describes the holding axis. Putting the timeout in the
  version lets a knowledge-axis parameter rewrite a holding-axis identity, and
  [[runtime-warrant]] is the whole argument for why those two must not touch.
- **The credential's value.** `Header::FromEnv` holds the *name* of an
  environment variable, and `version()` never reads it. A version that moved when
  a token was rotated would report every anchor behind that endpoint as read by a
  different instrument, and `Incomparable` would bury the corpus on the day
  somebody did the responsible thing. A test rotates the value and asserts the
  version does not move.

The value is resolved in `sent`, at call time, and goes into the request. It goes
nowhere else — **a `ProbeError` is written to the journal verbatim**, so a secret
in an error string is a secret committed to an append-only log that nothing can
delete. A test sends a credential, forces a failure, and greps the error for it.
What the error may say is the variable's *name*, which is the useful half.

The url got the same treatment in G1.5, and later than it should have: every
status branch below used to interpolate `ask.url`, which carries query strings,
tenant names and sometimes a credential of its own. They name the probe now, and
a url with userinfo in it is refused rather than fetched. See
[[transport-given]].

## Four HTTP statuses, three different people's problem

This is constraint 4 — never let unreachable read as an answer — at the only
layer that can tell the cases apart:

```
2xx        -> read the body
404 · 410  -> Outcome::NotFound      the endpoint answered: it is not there
401 · 403  -> ArtifactInvalid        our credentials, which retrying never fixes
5xx        -> Unreachable            establishes nothing either way
other      -> Unusable               neither an answer nor an outage
```

Folding 404 into an error would file the world's answer under our failures, and
the anchor would back off and retry a fact that is settled. Folding 5xx into
`NotFound` is the OCSP mistake the plan cites by name.

**A selector that matches nothing is `NotFound`, not an error.** The endpoint
answered and the field is not in the answer — the same as a file that exists
without the symbol in it. It is tempting to call it a broken selector, but a
wrong path and an absent field are indistinguishable from here, and guessing
which one it is would be inventing a diagnosis.

## Why the selector is on the probe and not left to the rules

Rule 6 gives the representation to the plugin and the attention to the anchor, so
reporting the whole body and letting rules pick would look more orthodox. It is
not: a crates.io payload carries download counts that move every few seconds, so
the fact address would change on every poll while the state did not — an
`Entry::Transition` each time, which is [[runtime-moved-at]]'s firehose arriving
through the probe. The selector says what this probe's subject *is*; the rules
still decide whether its moving matters.

`pointer` converts `$.a.b` and `a.b` into RFC 6901 and hands off to
`serde_json`'s own `Value::pointer`. The path syntax is a courtesy to whoever
types it; the resolution is not ours to reimplement.

## Why this does not reuse the http client next door

`batteries/provider/src/http.rs` has a `Fetch`-shaped trait and a reqwest
implementation already. It is `pub(crate)`, and it returns `ContentError`.
That difference is not an accident of visibility — it *is* the boundary:
`gmr-content`'s errors are about reaching a record, `gmr-probe`'s are about
reaching a fact, and the two taxonomies are what let `Footing` and `Warrant`
stay separate axes. Sharing the client would mean one battery depending on
another to save about fifteen lines of reqwest, and would put a single error
type across a line the architecture spends real effort keeping apart.

## When this changes, ask

Does something start being hashed that a caller can change without changing the
answer — a timeout, a retry count, a credential's value? Then anchors read under
identical criteria will start reporting `Incomparable` at each other.

Does a status stop being classified and start being passed through? Every one of
the four rows above is a different person's problem, and collapsing any two of
them tells a reader to go and fix the wrong thing.

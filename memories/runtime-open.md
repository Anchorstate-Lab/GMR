---
about:
  - crates/gmr-runtime/src/open.rs#OpenRequest
  - crates/gmr-runtime/src/open.rs#open
  - crates/gmr-runtime/src/open.rs#blind
  - crates/gmr-runtime/tests/operations.rs#an_anchor_whose_rules_read_what_its_probe_never_reports_is_refused_at_open
  - crates/gmr-runtime/tests/operations.rs#a_probe_that_cannot_say_what_it_reports_does_not_refuse_anything
watch: [sig, logic]
---

# Opening writes the log entry first; everything after it degrades to a warning

`OpenRequest.settings` is a plain `RunSettings`, stored outside the sealed
`Anchor` and changeable afterward with no rationale required — see
[[anchor-RunSettings]] for why that field never gets sealed in the first
place.

`open` resolves the probe before invoking it specifically so a name
nothing provides surfaces as a typo (`CannotOpen`) rather than being
allowed through to open an anchor that could only ever fail identically on
every future observation — refusing early is strictly better than refusing
forever, one observation at a time.

Opening an anchor that records digests only over a probe that answers in
plaintext is refused for the same reason, and by the same guard `observe`
passes through ([[anchor-recorded]]): the refusal is what makes the mode worth
declaring, so the one write path that could have opened without it does not
get to.

It also refuses an anchor whose rules read a field the probe **declares it never
reports** ([[probe-Derivation]]). That anchor is the worst shape this system can
take: it observes on schedule, its guard is never satisfiable, it never
transitions, and every verb that lists it reports a healthy settled anchor. The
first symptom is somebody eventually noticing that nothing has ever happened.

The refusal names both halves — what the rules read and what the probe reports —
because a refusal that only says no is a refusal somebody works around. And it
happens **before** the append: an anchor that exists and can never move is worse
than one that was never opened.

`bind_warnings` is not the same check and does not replace it. That one binds
the rules against the first real observation, so a field the probe happens not
to have emitted this once warns, and a field present this once and never again
passes. This one reads the declaration, which is the thing that cannot vary by
sample. A probe that answers `Observes::Unknown` refuses nothing, which is most
of them, which is why the warning stays.

Failing to compute an initial *state*, though, is not itself a reason to
refuse opening: an anchor can legitimately precede whatever it is
watching, in which case the transition table naturally resolves to
nothing yet. At the moment of opening, "the rules are wrong" and "the
target hasn't grown into existence yet" look identical — both get
recorded as a warning and surface loudly the first time a real observation
actually needs to evaluate the rules, not before.

Once `log.append` for `Entry::Open` succeeds, the anchor exists —
**the anchor is that log entry**, full stop. The journal and the queue are
two stores with no shared transaction between them, so `open` treats
everything after the append as a recoverable side branch: if
`scheduler.set_settings` fails, the anchor still opened, just running on
deployment defaults until sync repairs it (settings are mutable and
unsealed, so this costs nothing permanent — see [[anchor-RunSettings]]);
if `scheduler.ensure_enqueued` fails, the anchor still opened, just not
yet scheduled for automatic observation. Either failure must surface as a
warning attached to a *successful* `Opened`, never as an error — reporting
"failed to open" here would send a caller into a retry that immediately
hits `AlreadyOpen` while the real gap (missing settings, or not enqueued)
stays unrepaired.

## `AlreadyOpen` stopped being advice

The emptiness check and the append are separated by a probe call, so two
opens racing on one key both used to see an empty log and both used to
write. That was not a duplicate entry problem: `fold` **replaces** its
accumulator when it meets an `Entry::Open`, so the second one silently
discarded every observation and every accumulated bit since the first.

The append now states `Expected::Head(0)` ([[store-journal-expected]]), so
the store refuses the second one whatever the check saw. The check stays
because it is what turns the refusal into a sentence naming the anchor,
rather than a head that moved.

## What an open request may say, and what it may not

`OpenRequest` deserialises, because [[node-sdk]] takes one over a wire. Two
fields are not taken at face value:

- **`transitions`** arrive as `{ when, to }` strings and go through
  `Expr::text` here. `Expr` carries the hash earned from its source, and a
  caller who could hand one in could hand in a hash that does not match the
  text — after which every later reading is compared against a declaration hash
  that describes nothing.
- **`supersedes.rationale`** arrives as text and is stored as bytes. The sealer
  hashes bytes; asking a JSON caller for an array of byte values would be asking
  them to do the encoding.

Everything else is `#[serde(default)]` and `deny_unknown_fields`: an anchor
opened with a misspelled field is an anchor watching something other than what
was asked for, and it looks healthy.

## When this changes, ask

Does the blind check start refusing on `Observes::Unknown`? That is a ban on
every shell and script probe, arriving as a refusal that reads like a bug in the
caller's rules.

Does a failure after the `Entry::Open` append ever propagate as an `Err`
instead of a warning on `Opened`? That would make "the anchor definitely
exists" and "opening failed" both true at once, which is exactly the
misreporting this design avoids.

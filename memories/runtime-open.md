---
about:
  - crates/gmr-runtime/src/open.rs#OpenRequest
  - crates/gmr-runtime/src/open.rs#open
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

## When this changes, ask

Does a failure after the `Entry::Open` append ever propagate as an `Err`
instead of a warning on `Opened`? That would make "the anchor definitely
exists" and "opening failed" both true at once, which is exactly the
misreporting this design avoids.

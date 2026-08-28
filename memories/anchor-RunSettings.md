---
about: crates/gmr-core/src/anchor.rs#RunSettings
---

# How it runs, not what it judges

`RunSettings` is deliberately **not** inside `Anchor`. No field here is an input
to the transition function, and none changes any conclusion drawn from the log:
`retain` only decides how densely the same state gets written down, `facts`
only decides whether what is written may be plaintext (see [[anchor-recorded]]),
`cadence_secs` only decides how often we go and look, and `budget_ms` only
decides how long one probe call may take before it refuses — a budget may
produce no answer and must never produce a shorter one, which is what keeps it
out of the earned version (see [[probe-budget]]).

Sealing them together with the criteria would mean that changing something no
judgment depends on still demands a sealed rationale. So they live in mutable
storage and never enter the log.

## When this changes, ask

Can the new field change the result of any transition? If it can, it is not one
of these — it is a criterion, it belongs in `Anchor`, and it has to accept
sealing. That is the only entry test this struct has.

`facts` passes it and is still the strongest lever here: set to `Digests` over a
probe that does not digest, an anchor stops advancing entirely. That is loud
rather than silent — every refusal is an `Attempt` in the log and `check`
reports the streak — and it stays unsealed for the reason `cadence_secs` does:
nothing about it changes how any reading was judged.

## `cadence_secs` being `None`

`None` does not mean "do not observe". It means "use the deployment's default
pace". Pace is a throttle, not a criterion — so it may be absent, may be changed
at any time, and needs no sealed rationale.

That reading is also why a *declaration* of these fields has to be able to stay
silent about one without asserting anything about it. Nothing here can be
reconstructed from a default, so a declaration that fabricates the fields it
cannot express destroys what it was never told — see [[cli-settings-declared]]
for what that cost and how the domain says it now.

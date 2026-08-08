---
about: crates/gmr-core/src/anchor.rs#RunSettings
---

# How it runs, not what it judges

`RunSettings` is deliberately **not** inside `Anchor`. Neither field is an input
to the transition function, and neither changes any conclusion drawn from the
log: `retain` only decides how densely the same state gets written down,
`cadence_secs` only decides how often we go and look.

Sealing them together with the criteria would mean that changing something no
judgment depends on still demands a sealed rationale. So they live in mutable
storage and never enter the log.

## When this changes, ask

Can the new field change the result of any transition? If it can, it is not one
of these — it is a criterion, it belongs in `Anchor`, and it has to accept
sealing. That is the only entry test this struct has.

## `cadence_secs` being `None`

`None` does not mean "do not observe". It means "use the deployment's default
pace". Pace is a throttle, not a criterion — so it may be absent, may be changed
at any time, and needs no sealed rationale.

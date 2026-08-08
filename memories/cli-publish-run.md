---
about: domains/coding/cli/src/verbs/publish.rs#run
watch: [logic]
---

# Publishing and naming are one CLI step, because an unnamed artifact is unreachable

`run` publishes the artifact and installs it under `name` in the same
call, rather than exposing them as two separate verbs. A published artifact
that nothing has installed a name for cannot be resolved by
`Artifacts::resolve` (see [[transport-artifacts-resolve]]) — it would just
be inert bytes sitting in the content-addressed store — so a publish step
with no naming step would leave the user with something not yet usable.

`--env` entries get folded into the manifest via `publish`'s `env`
parameter, which is exactly what enters the hashed manifest and therefore
the derivation closure (see [[transport-manifest-address]],
[[transport-shell-derivation]]) — declaring an env var here is a real
statement about what the rule depends on, not a runtime convenience that
happens to also get recorded.

## When this changes, ask

Does the CLI gain a way to publish without installing a name, or install a
name for something not freshly published? Either one reopens the
unreachable-artifact gap this single-step design closes.

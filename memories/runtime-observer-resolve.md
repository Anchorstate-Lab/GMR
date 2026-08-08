---
about: crates/gmr-runtime/src/observer.rs#resolve
watch: [sig, logic]
---

# An unresolvable probe name is `Unusable`, not `Unreachable`

`resolve` classifies a name no transport recognizes as `Unusable`
(`ArtifactInvalid`), not `Unreachable`. The distinction matters downstream:
`Unreachable` means the world was asked and didn't answer, which is a
retryable, transient situation; an observation attributed to a probe name
that cannot even be resolved is worse than a missing observation, because
recording it as `Unreachable` would suggest retrying might help when the
real problem is a configuration mismatch that retrying cannot fix — the
name itself is not run at all.

## When this changes, ask

Does the new failure still distinguish "the world didn't answer" from "we
don't even know what probe this name refers to"? Conflating them into one
reason class would make callers retry a class of failure that retrying
cannot resolve.

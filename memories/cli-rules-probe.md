---
about: domains/coding/cli/src/rules.rs#probe
watch: [sig, logic]
---

# What the CLI takes from the user is a name, never a version

`probe` parses the user-supplied kind/name/params into a `ProbeRef` that
names a probe by `ProbeName`, with no version attached anywhere in this
path. The version is earned later, by resolving that name through a
`Transport` (see [[transport-contract]]) — it is never something a person
types in. Accepting a version here would let upgrading the tool (or a
probe's implementation) look like the user changed their mind about which
probe to use, when nothing about their declared intent changed at all.

## When this changes, ask

Does the CLI surface gain a way to pin a specific probe version from the
command line? That would conflate "which probe the user means" with "which
build of it happens to exist right now" — the two have to stay separate.

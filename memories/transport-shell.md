---
about:
  - batteries/transport/src/shell/mod.rs#Shell
  - batteries/transport/src/shell/mod.rs#invoke
  - batteries/transport/src/shell/mod.rs#a_probe_that_is_not_installed_is_our_failure
watch: [sig, logic]
---

# `Shell` only runs what the install index names, and never runs on a resolution failure

`Shell` executes artifacts, not arbitrary paths: an anchor names a probe by
`ProbeName`, and `Artifacts::resolve` (see [[transport-artifacts-resolve]])
says which artifact stands for that name here — verified byte for byte
before anything runs. If resolution fails, `invoke` classifies that as
`Unusable`, not `Unreachable`: we should not attempt to run a rule we
cannot even name, which is a different situation from a rule we named but
that then failed to answer. `a_probe_that_is_not_installed_is_our_failure`
is what pins this down: a probe name nothing is installed under is our
failure to configure, and it must never fold into the journal as if the
world had answered `NotFound`.

`invoke`'s child process starts from a cleared environment
(`env_clear()`), then only `LC_ALL`, `LANG`, `PATH`, and whatever the
manifest's own `env` map lists get added back. What actually runs is
exactly what the manifest declares — nothing inherited from this process's
environment sneaks in unaccounted for — and that same manifest `env` is
what `resolve`'s `verifiability` check reads to decide `Closed` vs `Open`
(see [[transport-shell-derivation]]).

## When this changes, ask

Does the child process's environment still come entirely from the
manifest plus the fixed baseline, with nothing inherited silently added?
Any inherited variable that is not in `resolved.manifest.env` breaks the
correspondence between "what the manifest declares" and "what actually
ran," which is what lets `resolve` claim `Closed` at all.

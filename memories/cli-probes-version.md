---
about:
  - console/cli/src/probes.rs#version
  - console/cli/src/probes.rs#source_hashes
  - console/cli/src/probes.rs#build_one
watch: [sig, logic]
---

# The recipe version is earned from tracked sources, and a missing one is fatal, not skipped

`Recipe::version` hashes tracked source bytes (via `source_hashes`) rather
than anything platform-specific, so the same recipe earns the same version
on every machine that builds it — see [[cli-probes-recipe]] for exactly
what does and does not enter that hash via `Record`.

`source_hashes` refuses outright when a declared source path does not
exist, rather than silently hashing whatever subset of sources it could
find. Hashing a smaller closure would mean a real change to the recipe's
criteria — a source file being deleted or renamed without updating the
declaration — could slip straight past the revision gate: the version
would still compute, just over less than it claims to cover, and nobody
would be told.

`obs` is the one part of the recipe deliberately outside this hash. It says what
the program reports rather than deciding it, so widening it leaves every reading
comparable — see [[transport-manifest-address]] for why that asymmetry is load
bearing. It is not therefore unguarded: it travels in the manifest and is inside
the address the artifact is stored under.

`build_one`'s published `version` is already the full semantic closure by
the time it reaches `publish`: it covers sources, entrypoint, args, env,
and the output contract, and deliberately excludes platform and the built
bytes. That is exactly what "derivation" means for a shell probe (see
[[transport-manifest-address]]) — `build_one` does not need to compute
anything further before calling `publish`.

## When this changes, ask

Does a new failure path in `source_hashes` return a partial hash instead
of erroring? Any way to compute *a* version for fewer sources than
declared reopens the "smaller closure slips past the gate" hole this
refusal exists to close.

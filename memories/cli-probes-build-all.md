---
about: console/cli/src/probes.rs#build_all
watch: [logic]
---

# `build_all` is the developer path; users install pre-built artifacts

`build_all` compiles/stages/publishes every declared recipe from source.
That is what a contributor working on this repository needs, but it is
not how a released binary reaches an end user — `gmr init` installs
artifacts that were already built at release time (see the bundled-probe
handling in `verbs::init::bundled`), so ordinary users never need a build
toolchain at all.

## When this changes, ask

Does a change here assume every user can run `build.sh`/compilers/etc.
locally? That assumption is only true for this repository's own
development, not for anyone who installed a released `gmr` binary.

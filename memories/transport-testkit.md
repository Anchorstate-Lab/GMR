---
about:
  - batteries/transport/src/shell/testkit.rs#install_script
  - batteries/transport/src/shell/testkit.rs#publish_script
  - crates/gmr-runtime/tests/chain.rs#cat_probe
  - crates/gmr-runtime/tests/grounding.rs#cat_probe
  - crates/gmr-runtime/tests/operations.rs#cat_probe
  - crates/gmr-runtime/tests/ordering.rs#cat_probe
  - crates/gmr-runtime/tests/replay.rs#World
  - crates/gmr-runtime/tests/state_machine.rs#script_probe
watch: [sig, logic]
---

# The testkit goes through real publish and install, and states a real closure

`install_script` calls the same `publish` and `Artifacts::install` a real
build uses, rather than faking an installed entry directly — "earned
versions" (see [[transport-artifacts-publish]]) is a guarantee about the
production path, and it would only be tested if the test path exercises
the same code. `publish_script` plays the publisher's role honestly too:
`publish` needs a `derivation` handed to it, and since a test has nothing
else to earn one from, it hashes the script body itself and passes that —
the smallest true closure available to a test, not a placeholder.

`observes` is the other thing a publisher states, and the testkit states
nothing: a test script's output shape is not declared anywhere, so an empty
`Observes` is the true answer. [[probe-Derivation]] is why that reads as
"covers everything" rather than "covers nothing" — a transport that cannot
say what it reports must not be taken to have promised anything narrow.

Downstream integration tests follow the same rule: `gmr-runtime`'s
`cat_probe` (in `tests/chain.rs`) calls `install_script` rather than
constructing a `ProbeRef` by hand, for the same reason — every layer of
test should exercise the real publish/install path, not just the
`gmr-transport` crate's own tests.

## When this changes, ask

Does the testkit still call the real `publish`/`install` functions, or has
it grown a shortcut that writes files directly? A shortcut here would let
tests pass while the production path silently regresses.

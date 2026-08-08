---
about:
  - batteries/transport/src/shell/testkit.rs#install_script
  - batteries/transport/src/shell/testkit.rs#publish_script
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

## When this changes, ask

Does the testkit still call the real `publish`/`install` functions, or has
it grown a shortcut that writes files directly? A shortcut here would let
tests pass while the production path silently regresses.

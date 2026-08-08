---
about: crates/gmr-core/src/probe.rs#address
---

# "The world says there is nothing here" needs an address too

NotFound gets an address as well. Without one, after swapping the derivation rule
from A to B, two NotFounds would compare equal — and "looked again with a
different probe, still nothing" and "nothing happened at all" would be
indistinguishable in the log.

The address is computed together with `derivation.version`, so the same "nothing
here" has a different address under a different probe. That is why `should_still`
can tell those two cases apart.

## When this changes, ask

Someone wants NotFound to use one fixed constant address (simpler, comparable) →
ask them: after swapping the probe and still not finding it, how do you plan to
tell that apart from not finding it without swapping the probe?

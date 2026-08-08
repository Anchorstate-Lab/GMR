---
about: crates/gmr-runtime/src/observe.rs#instrument
watch: [sig, logic]
---

# `instrument` answers identity without running anything, so a swap is knowable early

`instrument` resolves what a probe declaration would derive right now, with
nothing actually invoked — it calls `Transport::resolve` (see
[[transport-contract]]), never `invoke`. Comparing this against an
anchor's last recorded derivation is how a swapped instrument (a changed
script, a different transport version) gets noticed on its own, distinct
from a moved world: if the derivation changed, something about *how we
look* changed, which is a different situation from the probe reporting new
facts through an unchanged derivation.

## When this changes, ask

Does the new check compare a derivation obtained by actually invoking the
probe, rather than by resolving it? Invoking to check for a swap would mean
paying for a reading just to answer a question that resolution alone can
answer for free.

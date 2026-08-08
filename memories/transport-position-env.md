---
about: crates/gmr-probe/src/lib.rs
watch: [sig]
---

# `POSITION_ENV` names the one channel both sides read

`POSITION_ENV` is the environment variable name a shell-probe transport uses to
hand a probe process its `state.position`. It is declared once here, in the
contract crate, specifically so that a transport implementation and a probe
script never each pick their own name for the same channel. Two copies of that
string — one in the caller, one in every probe script — would drift the moment
either side got edited alone, and nothing would notice until a probe read the
wrong variable and got `NotFound` for a position that really exists.

## When this changes, ask

Does every shell-probe transport (`gmr-transport` and any other Transport
impl that shells out) read this same constant rather than a hardcoded string?
Renaming it without checking breaks every probe script silently — they would
just see an unset variable, not an error naming this constant.

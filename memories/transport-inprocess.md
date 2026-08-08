---
about:
  - batteries/transport/src/inproc.rs#Extract
  - batteries/transport/src/inproc.rs#Registered
  - batteries/transport/src/inproc.rs#InProcess
watch: [sig, logic]
---

# `InProcess` earns `Verifiability::Closed` by construction, not by assertion

`Extract` is `(cwd, position, params) -> facts | null` — the same contract a
subprocess probe answers on stdout, minus the process boundary. `Registered`
pairs one such function with the hash of everything that can change what it
returns. `InProcess` itself does not decide which probes exist or what each
one's version closure covers; that is the assembly's call, this type only
carries the map it was handed.

`Verifiability::Closed` is safe to return unconditionally from `resolve`
only because of a structural fact, not a runtime check: the `version` handed
back is the very `Registered::version` of the same entry `invoke` will then
call. There is no way for `resolve` to answer with one probe's identity and
`invoke` to run a different one — they read the same map entry.

## When this changes, ask

Does `resolve` and `invoke` still definitely read the same `Registered`
entry for a given name? Any change that lets them diverge — a cache, a
different lookup path — invalidates the "Closed by construction" claim and
`Verifiability::Closed` would then need a runtime check to still be true.

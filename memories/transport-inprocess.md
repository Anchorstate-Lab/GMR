---
about:
  - batteries/transport/src/inproc.rs#Extract
  - batteries/transport/src/inproc.rs#Registered
  - batteries/transport/src/inproc.rs#InProcess
watch: [sig, logic]
---

# `InProcess` earns `Verifiability::Closed` by construction, not by assertion

`Extract` is `Reach -> facts | null` — the same contract a subprocess probe
answers on stdout, minus the process boundary. `Reach` carries the cwd, the
position and the params a subprocess would have read from its environment, plus
the `Budget` a subprocess gets for free by being killable: it is the one slot
through which anything a call needs reaches the work, so widening it later costs
a field rather than a signature every implementor has to follow. What comes back
is `Result<Value, ExtractError>`, and `ExtractError` separates a budget that ran
out from a refusal, because the two become different `FailureCode`s in the
journal and a cancelled scan recorded as an ordinary failure would be a lie.

`Registered` pairs one such function with the hash of everything that can change
what it returns. `InProcess` itself does not decide which probes exist or what
each one's version closure covers; that is the assembly's call, this type only
carries the map it was handed.

Note what is deliberately *not* in `Reach`: nothing that changes the answer.
Adding the budget did not move any extractor's version, and must not — a
deadline decides whether there is an answer, never which answer it is. See
[[survey-narrow]] for the same line drawn on the other side.

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

---
about:
  - console/core/src/lib.rs#opened
  - console/core/src/lib.rs#asking
  - console/core/src/lib.rs#Opening
watch: [sig, logic]
links:
  rests-on: [node-sdk]
---

# The console has one shared half, and every door adapts only the error type

`gmr-console` is what fell out of writing the second binding: assembling a
`Runtime` from an options object, parsing claims and asks, spelling the
refusals — all of it was about to exist twice, once under napi and once under
pyo3, and two copies of `named`'s grammar is two grammars the day one is
edited. The core keeps every decision (`Opening`'s shape, the transport roster
a door assembles, the wording of each refusal) and returns `Fault = String`;
node maps it to `napi::Error`, python to `PyErr`, and neither door holds any
logic a `grep` of this file would miss.

The verb table itself stays in the doors on purpose: which verbs a transport
serves is that transport's visible surface, one napi/pyo3 method per console
verb, thin enough that the table is readable as a listing. What must never
appear here is anything from the pack table — `check`'s subscriptions,
`sync`'s declaration channel — the line [[node-sdk]] draws for every door at
once.

## When this changes, ask

Does a door start carrying parsing or assembly of its own — a claim grammar
tweak, an extra transport, a differently-worded refusal? That is the two-copy
drift this crate exists to prevent; the change belongs here, once. Does this
crate start deciding delivery or criteria? That is pack policy arriving in the
console wearing a helper's clothes.

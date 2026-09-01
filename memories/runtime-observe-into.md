---
about: crates/gmr-runtime/src/observe.rs#observe_into
watch: [sig, logic]
---

# The fact address comes from the rule that derived it, and absence gets one too

`observe_into` addresses the outcome using `derivation.version` — the
version the probe actually resolved to for this call — never the
declaration on the anchor. Those two can disagree (a swapped instrument,
see [[runtime-instrument]]), and the address has to reflect which rule
actually ran, not which rule the anchor merely names. `Outcome::NotFound`
gets addressed the same way as `Found`: absence is itself an answer from a
specific derivation, not a lack of one, so it is not exempted from having a
`fact_address`.

This is also the only place an `Outcome` becomes an `Observation`, which is
why the digests-only guard sits here rather than beside either append — see
[[anchor-recorded]]. Addressing and admitting are two jobs in one function
because they are the same moment: the point where a reading is accepted as
something the log may hold.

## When this changes, ask

Does the new code address a fact using the anchor's declared probe instead
of the resolved `derivation`? And does `NotFound` still get a real
`fact_address` rather than being skipped as "nothing to address"?

Does a second way to build an `Observation` appear? Then the guard has a
bypass, and `docs/ARCHITECTURE.md` §11.4's rule about the premise guard covering *every*
write path is the argument against it.

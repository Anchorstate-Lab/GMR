---
about:
  - crates/gmr-runtime/src/open.rs#Supersede
  - crates/gmr-runtime/src/open.rs#seal_supersede
  - crates/gmr-runtime/tests/state_machine.rs#a_new_generation_supersedes_the_finished_one_with_a_sealed_reason
watch: [sig, logic]
---

# Superseding needs a sealed rationale, and the old anchor has to already be closed

`Supersede.rationale` is not `Option<Vec<u8>>` — it is mandatory, because
superseding an anchor is a change of heart about criteria, and a criteria
change is exactly the kind of decision this system seals a rationale for
(unlike `RunSettings`, see [[anchor-RunSettings]]). `seal_supersede` refuses
to proceed (`NotClosedYet`) unless the anchor being superseded has already
folded to `closed`, because two generations of the same lineage running at
once would be a way to route around ever actually finishing the old one —
"supersede" has to mean succession, not parallel operation.

`a_new_generation_supersedes_the_finished_one_with_a_sealed_reason` checks
both halves of this: the rationale is retrievable through the same sealed
chain `revise`/`close` use (not a separate, unaudited side channel), and
the old generation's anchor stays `closed` afterward — superseding creates
a new lineage member, it does not resurrect the old one.

## When this changes, ask

Does the new path let `supersedes` point at an anchor that is still open?
Allowing that turns supersession into a bypass of closing, which defeats
the reason superseding requires a sealed rationale in the first place.

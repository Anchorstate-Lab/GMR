---
about:
  - crates/gmr-runtime/src/open.rs#Supersede
  - crates/gmr-runtime/src/open.rs#seal_supersede
  - crates/gmr-runtime/src/bind.rs#living
  - crates/gmr-runtime/src/bind.rs#heir_of
  - crates/gmr-runtime/tests/state_machine.rs#a_new_generation_supersedes_the_finished_one_with_a_sealed_reason
  - crates/gmr-runtime/tests/state_machine.rs#an_assertion_naming_a_superseded_generation_lands_on_the_living_one
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

## The edge is read in both directions, and only one of them is an edge

`supersedes` points from the heir to the generation it replaced, so reading
a chain of ancestors is a walk. Nothing points the other way. To answer
"who superseded this", `heir_of` scans every anchor and folds it — there is
no reverse edge to follow.

`living` runs that scan only when the named anchor has folded to `closed`,
which is the only state that can have an heir, so the ordinary bind pays one
journal read and nothing more. If the scan ever does hurt, the fix is a
**derived index** — a projection rebuildable from the journal — not a stored
reverse edge. A second edge is a copy, and a copy drifts.

Both directions are load-bearing. Delivery walks backwards so a memory left
on a superseded generation still reaches the reader through the heir
([[store-orset-projection]]). Binding walks forwards so a new assertion
lands on the generation that is actually standing — without which every
self-bind would arrive on a dead ancestor and reach the present only by
being carried, and a superseded generation would stay writable, letting a
tag added there slip past a revocation made on the heir.

## When this changes, ask

Does the new path let `supersedes` point at an anchor that is still open?
Allowing that turns supersession into a bypass of closing, which defeats
the reason superseding requires a sealed rationale in the first place.

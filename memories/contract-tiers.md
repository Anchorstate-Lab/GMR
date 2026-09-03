---
about: crates/gmr-runtime/src/contract.rs
watch: [grew, shrank, roll]
---

# The contract has two tiers, and only one of them earns the churn

Eleven of the first twelve contract bumps landed in one week, and almost every
one of them changed the same half of the surface. The re-export roster splits
by what moves and who is hurt when it does:

**Core — request shapes and identity vocabulary.** `Binding` · `Claim` · `Ref`
· `SaidId` · `Source` · `Version` · `FactAddress` · `Expr` · `LinkKind` ·
`Openness` · `Observes` · `Verifiability` · `Derivation` · `Instructions` ·
`Asked` · `OpenRequest` · `Supersede` · `Opened` · `Landed`. What a caller
*sends*, and the names things are pinned by. These settle early and move
rarely; a change here is almost always breaking, because the runtime refuses
what it cannot parse.

**Report — the answer vocabulary.** `Depends` · `Holding` · `Shown` ·
`Knowledge` · `Blind` · `Warrant` · `Evidence` · `Anchored` · `Standing` ·
`Grounding` · `Before` · `Footing` · `Reading` · `Grounded` · `AnchorView` ·
`MemoryView` · `SaidView` · `Linked` · `Inbound` · `Links` · `Reached` ·
`Edge` · `Edges` · `Raised` · `ContentErrorCode`. What the runtime *says
back*. This is where the vocabulary is still being earned, and where every
additive segment of `gmr.contract.v<breaking>.<additive>` will move.

The two-segment version and the fallback-arm rule in the declaration headers
are the cheap treatment: additive report growth stops being a breaking event
by convention. The expensive treatment — two version strings, a frozen core
contract and an evolving report contract — is deliberately **not** taken yet.
Splitting is itself one more breaking change for anyone pinning the string,
and with every consumer living in this repository, it would purchase nothing.

**The trigger to revisit:** the first consumer outside this tree that pins
`gmr.contract.*`. At that moment, decide the split with them in the room —
which report types they actually match on is evidence no audit from in here
can produce. [[runtime-ground]] already holds the principle that rendering
carriers do not belong behind an earned hash; `MemoryView` and `AnchorView`
sit in the report tier as the first candidates that argument would evict.

## When this changes, ask

Did a type move between tiers, or arrive unclassified? The roster above is
this note's whole claim — a re-export this note does not file is a shape
nobody decided the churn budget for.

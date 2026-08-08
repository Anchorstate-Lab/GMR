---
about: batteries/survey/src/matching.rs#roll
watch: [sig, logic]
---

# A roster compares who is present, not what they look like

`roll` is the identity list of the tied set: one per line, sorted. A roster
compares that against its baseline rather than the full report of every candidate
— so **changing a function body does not move the roster**, because no identity
changed. This axis is called `swapped`: it is the only one that lights up when
members come and go while the total stays the same (see [[layers]]).

**Duplicates are kept**, so `roll.lines().count() == candidates` always holds.
Deduplicating would collapse every candidate the extractor cannot name into the
same empty line, and the roster would undercount while saying nothing about it.
This is not hypothetical: `layer::gmr`'s public surface is all `pub use`, and
`use_declaration` has no `name` field, so those five candidates were all empty
strings for a while (see [[ast-signature]]).

The fix that time was **to give them real names** (the `argument` field, i.e. the
import path itself), not to invent a `kind:@byte-offset` fallback. A byte offset
changes on any edit above it, which is just relocating the hair-trigger.

**An id that is unstable under unrelated edits is worse than dropping the
candidate. Neither is the answer; filling the gap in the representation is.**

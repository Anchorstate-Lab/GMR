---
about: batteries/survey/src/cache.rs#visit_cached
watch: [sig, logic]
---

# The cache key must cover everything that can change what `collect` returns for a file

`Cache`'s whole reason to exist is "same input, skip recomputing" — which only
holds if the key genuinely captures every input `collect` reads. Right now that's
`probe` name, `root` (folded into `scope`), and each file's content hash. Nothing
else reaches `collect`: `probe(root, pos, cache)` never passes `params` through,
so `root` really is the only other axis besides file content.

It was not always in the key. The first version keyed the scan-level memo on
`probe` alone. `root_of` takes `root` out of `params`, not the process's cwd — see
[[extract-routing]] — so two anchors can call the same probe with two different
roots. The `layer::*` anchors do exactly that, narrowing `ast-map` to a single
crate each via `params: {root: crates/X}` (see [[layers]]). With `root` missing
from the key, whichever `layer::*` anchor's `ast-map` call ran first in a `gmr
check` cached its candidate list under `"ast-map"`, and every other `layer::*`
anchor — different crate, different intended root — was served that same list.
All six rosters collapsed to one identical (wrong) `now.roll`. `gmr check` caught
it as `grew` on all six at once; reading `now.roll` showed the six were no longer
distinct from each other, though each still differed from its own `baseline.roll`.

## When this changes, ask

Does `probe`'s signature start reading anything from `params` besides what
`root_of` already extracts, or does `collect` start depending on anything besides
`(root, path, file bytes)`? If so, that new input has to fold into `scope` (or a
new dimension of the key) the same way `root` did — otherwise this is the same
bug again, and nothing will catch it except an anchor that happens to narrow scope
the way `layer::*` does, and a person who happens to read `now.roll` closely
enough to notice six lists are identical instead of merely wrong.

---
about: domains/coding/extract/src/lib.rs#narrow_of
---

# The tree to walk and the subtree to narrow to are different axes, not one joined path

`root_of(cwd, params)` used to join the CLI's repository root with an
anchor's `params.root` into one absolute path, and every extractor's
`probe()` took that single joined path as both "where to walk" and "what to
report on". That is the exact shape [[survey-cache-scope]] measured as a
bug: seven `layer::*` anchors narrowing `ast-map` to seven different crates
meant seven different joined roots, so the old `Cache` (keyed by
`probe@stamp@root`) stored the same file's content once per layer anchor —
4.5 MB of distinct content held as 6.5 MB, growing with every anchor
somebody opens.

`Bridge`'s `tree` is fixed once, at construction (`registry()` now takes
`root: &Path` for exactly this). `narrow_of(params)` extracts `params.root`
alone — no `cwd` joined in — and that plain relative string is what reaches
`look()`'s `root: &str` parameter, where it narrows a query against the one
already-built index rather than re-scoping a second walk. Opening a new
`layer::*` anchor now costs zero index bytes: it asks a different question
about facts that already exist, it does not create new ones.

`registry_uncached()` could not get the same treatment. Its own tests
(`domains/coding/extract/src/lib.rs`'s `tests` module) reuse one registry
across several different `reach.cwd` values — a fresh temp dir per test —
which a `Bridge` with one fixed tree cannot serve. Its real caller
(`probes::list`) only ever reads `.version` off the result and never calls
`.extract` at scale, so its closures build a fresh throwaway in-memory
`Bridge` scoped to that call's own `reach.cwd` instead — matching
"uncached" literally, and matching what `root_of` used to do per call
anyway, just without the join.

## When this changes, ask

Does any extractor or call site reach for `reach.cwd` and `params.root`
together again, joined into one path? That is the old bug's exact shape —
the tree a `Corpus` walks must come from where it was constructed, never
from a per-call join with the narrowing string.

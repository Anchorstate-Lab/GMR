---
about: packs/coding/extract/src/lib.rs#narrow_of
---

# The tree to walk and the subtree to report on are two axes, never one joined path

`Bridge`'s `tree` is fixed once, at construction — `registry()` takes
`root: &Path` for exactly that. `narrow_of(params)` extracts `params.root`
alone, with no `cwd` joined into it, and that plain relative string reaches
`look()`'s `root` parameter, where it narrows a query against the one
already-built index.

Keeping them apart is what makes a narrowing anchor free. The `layer::*`
anchors point `ast-map` at one crate each ([[layers]]); they ask a different
question about facts that already exist rather than creating new ones, so
opening another one costs zero index bytes. Joining the two axes into a single
path would make each narrowing its own walk and its own stored copy of every
file underneath it, and the duplication grows with every anchor somebody opens.

It is also what lets `root` be a predicate rather than part of the address —
see [[survey-index-shape]] for why `under(rel, root)` decides containment and
the generation does not mention the root at all.

`registry_uncached()` builds a throwaway in-memory `Bridge` per call, scoped to
that call's own `reach.cwd`. Its real caller reads `.version` off the result and
never extracts at scale, and its tests reuse one registry across a fresh temp
dir per test — which one fixed tree cannot serve.

## It comes from params, so the anchor can state what it meant

`params` enter the declaration hash, so an anchor records the subtree it was
always about. A process's working directory differs with whoever runs it: the
same anchor would observe two different trees on two machines, the logs would
not line up, and nothing anywhere would record that they disagreed.

The seven `layer::*` anchors narrow to a single package exactly this way, with
`params: {root: crates/X}`.

## When this changes, ask

Does any call site reach for `reach.cwd` and `params.root` together and join
them? The tree a `Corpus` walks comes from where it was constructed; a per-call
join puts the walk back on the query path, where every narrowing pays for a
scan of its own.

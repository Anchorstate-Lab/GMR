---
about:
  - domains/coding/extract/src/lib.rs#declares
  - domains/coding/extract/src/lib.rs#catchall
  - domains/coding/cli/src/probes.rs#for_extension
  - domains/coding/extract/src/lib.rs#root_of
watch: [sig, logic]
---

# Extension routing and "which part to look at" — neither may come from the process

## Routing is asked in two halves so a specific answer can outrank a general one

`coord::route` takes the coordinate's extension and asks the catalog; the probes
declare for themselves what they answer to. So nowhere in the CLI does the word
"rust" or "typescript" appear.

The catalog asks two different questions, and they cannot be one call. `declares`
answers "does a builtin name this extension"; `catchall` answers "which one probe
reads anything at all". Between them the catalog asks the repository's own
`.anchor/probes.toml`:

```
builtin declares → script handles → recipe handles → builtin catchall
```

Collapsing those two halves into one builtin lookup is what shadowed every
declared probe: the catchall claims every extension, so asking the builtin roster
as a whole always answered first, and a repository that installed a probe for
`.sh` silently got the whole-file fingerprint instead — the instrument swapped
under the anchor with nothing said. **Specific beats general is a rule across
sources, not within one of them.**

Which probes are even reachable this way is derived, not listed: `addressable(at)`
keeps the ones whose vocabulary has a `file` or `path` slot, which is what a
person pointing at a whole file means. `name-map`'s coordinate is `(name, scope)`
and it drops out by itself. The one addressable probe whose `Reads` is `Anything`
becomes the fallback without anybody naming it, and
`at_most_one_addressable_probe_may_read_anything` refuses a build where two
candidates would let iteration order decide.

prose-map wants a `heading`, so a `file#part` coordinate lands there only when the
part is a heading; `wanted` drops a `name` it has no slot for and the anchor ends
up watching the whole file, which `report` says out loud as `missed`.

## The part to look at comes from params, not from the process

`root_of` takes `root` out of params rather than deriving it from the current
working directory. params enter the declaration hash, so **an anchor can state what
it originally meant**; a process's cwd differs with whoever runs it, the same
anchor would observe two different trees on two machines, the logs would not line
up, and nothing anywhere would record the difference.

Those six `layer::*` anchors narrow their scope to a single package exactly this
way, with `params: {root: crates/X}` (see [[layers]]).

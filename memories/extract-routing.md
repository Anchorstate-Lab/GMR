---
about:
  - packs/coding/extract/src/lib.rs#declares
  - packs/coding/extract/src/lib.rs#catchall
  - console/cli/src/probes.rs#for_extension
watch: [sig, logic]
---

# Routing asks two halves, so a specific answer can outrank a general one

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

Which subtree an anchor then reports on is a separate axis that also may not
come from the process — see [[lib-narrow_of]].

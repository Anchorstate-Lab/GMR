---
about:
  - domains/coding/extract/src/lib.rs#for_extension
  - domains/coding/extract/src/lib.rs#root_of
watch: [sig, logic]
---

# Extension routing and "which part to look at" — neither may come from the process

## `handles` is what lets `about:` route, and the CLI knows no language names

`coord::route` takes the coordinate's extension and asks `for_extension`; the
probes declare for themselves which ones they answer to. So nowhere in the CLI
does the word "rust" or "typescript" appear. An empty `handles` means this probe
does not travel that road — it has to be named explicitly in long-hand form.

Only ast-map can eat `path#name`: that coordinate shape produces `{file, name}`,
and no other probe's vocabulary matches. prose-map wants a `heading`, so either
name it explicitly, or `wanted` drops the `name` and the anchor silently ends up
watching the whole file.

## The part to look at comes from params, not from the process

`root_of` takes `root` out of params rather than deriving it from the current
working directory. params enter the declaration hash, so **an anchor can state what
it originally meant**; a process's cwd differs with whoever runs it, the same
anchor would observe two different trees on two machines, the logs would not line
up, and nothing anywhere would record the difference.

Those six `layer::*` anchors narrow their scope to a single package exactly this
way, with `params: {root: crates/X}` (see [[layers]]).

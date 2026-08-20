---
about:
  - batteries/transport/src/shell/artifact.rs#install
  - batteries/transport/src/shell/artifact.rs#installed
  - batteries/transport/src/shell/mod.rs#reinstalling_a_name_repoints_it
watch: [sig, logic]
---

# Installing a name overwrites its old pointer on purpose

`install` overwrites whatever version a name previously pointed to, rather
than refusing when the name is already installed: refusing would leave the
machine running the old binary forever, which is worse than the small risk
of an overwrite. `reinstalling_a_name_repoints_it` is what pins this down —
publishing a second version under the same name must change what
`installed()` returns for it, not add a second entry or refuse.

`installed` returning `Ok(None)` is a real, complete answer: a syntactically
good name that simply is not installed here. That is different from an
`Err` — which means the index itself could not be read or trusted — and
different from a stale bad state that a caller would need to repair.

That line only holds because the index is typed (see
[[transport-artifacts-store]]). While it held strings, "the index cannot be
trusted" had a third outcome that was neither: a value that was not a version
came back as `Ok(Some(..))` and was believed.

## When this changes, ask

Does a repeated `install` for the same name still leave exactly one
version installed under it (the newest), rather than accumulating or
refusing? And does "not installed" still surface as `Ok(None)` rather than
an error a caller has to specifically distinguish from "the index is
corrupt"?

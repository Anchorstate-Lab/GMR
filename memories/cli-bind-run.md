---
about: domains/coding/cli/src/verbs/bind.rs#run
watch: [logic]
---

# The pre-check is git-only; versioning always goes through the registered provider

`run`'s repo-tree existence check (`root.join(&path).exists()`) only means
something for the `git` provider, which resolves paths against this
checkout. Other providers resolve `path` against their own root and report
absence themselves — through `current_version` returning `None` — so the
check is skipped for anything but `git` rather than generalized into a
check no other provider's paths would satisfy.

`current_version` is called here rather than reaching for the git backend
directly, because it is the same function `read` and `edges` use to decide
"what version does this reference have right now" (see
[[runtime-current-version]]). A second, separate lookup here could drift
from what those verbs report for the same reference.

## When this changes, ask

Does a new provider need its own existence pre-check bolted onto this
function? It should not — the provider's own `current_version` returning
`None` is already the generic "does not exist here" answer this code
relies on for every provider but `git`.

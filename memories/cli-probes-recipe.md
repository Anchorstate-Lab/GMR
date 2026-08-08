---
about:
  - domains/coding/cli/src/probes.rs#Obs
  - domains/coding/cli/src/probes.rs#Recipe
  - domains/coding/cli/src/probes.rs#ScriptDecl
  - domains/coding/cli/src/probes.rs#Record
watch: [sig]
---

# What's inside the recipe's hash, and what deliberately isn't

`Obs` — the schema, `at`, and `facts` a probe emits — sits outside the
recipe's version on purpose: changing which shapes a probe's output fits
is not a change to the derivation rule itself, it is metadata about how to
interpret an unchanged rule's output. `Recipe.handles` (file extensions
`about:` can route through) is excluded from the version for the same
reason: it lets the CLI route a coordinate to a probe without the CLI
knowing any language's name, and it changes nothing about what the probe
actually derives.

`Recipe.build` is empty for a script probe — there is no build step,
staging the file is enough. `Recipe.env_from_host` names host environment
variables pulled into the closure at build time; using any of them
downgrades the resulting artifact's verifiability, because the value came
from this machine's environment rather than from tracked, comparable
sources (see `env_from_host` on the `test-roster` probe declaration for a
worked example).

`ScriptDecl` describes a probe that is nothing but a file in this
repository: `run` is the path, and the file's own content — hashed when
the probe is actually called, not once at declaration time — is its
identity.

`Record` is the exact shape `Recipe::version` hashes (see
[[cli-probes-version]]): platform, the built binary's own hash, and any
captured host env value are deliberately absent from it, because those
three are precisely what would make an artifact's version local to one
machine instead of comparable across machines building the same recipe.

## When this changes, ask

Does the new field change what the probe actually derives, or only how
its output gets routed/interpreted? Only the former belongs inside
`Record`'s hash; the latter (like `Obs`, like `handles`) stays outside it.

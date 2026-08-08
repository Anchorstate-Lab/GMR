---
about:
  - batteries/transport/src/shell/mod.rs#Published
  - batteries/transport/src/shell/mod.rs#the_version_is_the_rule_not_the_bytes_that_implement_it
  - batteries/transport/src/shell/mod.rs#host_env_opens_the_closure
watch: [sig, logic]
---

# The journal records the derivation, not the artifact address

An artifact has two distinct hashes, which the test helper `Published`
keeps apart on purpose: `address` is where the published bytes live
(the manifest hash — depends on exact file layout), and `derivation` is
what rule it implements. `Shell::resolve` hands back `derivation` as the
`Derivation::version`, never `address` — two machines that build the same
probe and end up with different bytes-on-disk (different compiler,
different timestamps embedded, whatever) still have to compare equal in
the journal as long as the rule is the same one, which is exactly what
`the_version_is_the_rule_not_the_bytes_that_implement_it` checks.

Whether that derivation counts as `Verifiability::Closed` or `Open` turns
on one thing: whether the manifest's `env` map is empty. Any host
environment variable that reaches the probe without itself entering the
hashed manifest keeps the closure honestly `Open`, however precisely the
artifact's bytes are pinned otherwise — `host_env_opens_the_closure` is
what exercises exactly that flip.

## When this changes, ask

Does a new way of parameterizing a probe (beyond `args` and `env`) get
folded into the manifest's hash, or does it reach the running process some
other way? Anything that reaches the process without being in the hashed
manifest has to flip verifiability to `Open`, the same way `env` does.

---
about:
  - batteries/transport/src/shell/manifest.rs#Manifest
  - batteries/transport/src/shell/manifest.rs#address
  - batteries/transport/src/shell/manifest.rs#the_platform_is_part_of_the_address
watch: [sig, logic]
---

# `Manifest` carries two hashes because they answer two different questions

`address` is the byte-exact identity of the files on *this* machine right
now — it moves with the platform, and it is what `Artifacts::resolve`
verifies against (see [[transport-artifacts-resolve]]). `derivation` is
what the publisher earned from sources, the same wherever the probe is
built, and it is what the journal records (see
[[transport-shell-derivation]]). Folding `platform` into `address` is what
keeps them different jobs: the same rule built for two platforms must
produce two different addresses (different bytes really are stored
separately) while still sharing one `derivation` — otherwise no journal on
one machine could be compared against a journal on another, which is
exactly what `the_platform_is_part_of_the_address` checks (`address`
differs, `derivation` does not).

`address` never risks exceeding the canonicalizer's depth limit, because a
`Manifest`'s nesting is bounded by its *type*, not by how much data it
holds — however many files, args, or env entries it carries, they sit at
the same fixed depth in the JSON shape. So `content_hash_of` is safe to
`expect()` here without a runtime depth check.

## When this changes, ask

Does the new field nest `Manifest` one level deeper conditionally (e.g. a
recursive or optional-of-optional structure)? If so the "bounded by type,
not by data" argument no longer holds and the `expect()` on
`content_hash_of` needs to become a real error path.

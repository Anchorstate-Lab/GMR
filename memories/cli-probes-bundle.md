---
about:
  - domains/coding/cli/src/verbs/probes.rs#bundle
  - domains/coding/cli/src/verbs/probes.rs#write_install_index
watch: [logic]
---

# A bundle ships only what is currently installed, and its index is rewritten, not copied

The working store under `store_dir` accumulates every artifact ever built
there, including ones for probes that are no longer declared. `bundle`
does not copy that store wholesale — it walks the current `recipes`,
resolves each one's currently-*installed* artifact, and copies only those
directories. A release tarball carrying every historical artifact would
grow forever and ship dead weight nobody asked for.

`write_install_index` builds a fresh `installed.json` from `shipped`
rather than copying the working store's own index file, for the same
reason: the working index also names artifacts that did not make it into
this bundle (superseded builds, retired probes), and copying it verbatim
would let the bundle's index claim artifacts the bundle does not actually
contain.

The built-in extractors are deliberately absent from this bundle — they
live in the binary itself (see [[extract-closure]]), so there is nothing
for `bundle` to ship for them.

## When this changes, ask

Does the new code copy the working store's install index or artifact
directory directly, instead of rebuilding a subset from `recipes` and
`shipped`? Copying directly would let the bundle claim artifacts it never
actually packaged.

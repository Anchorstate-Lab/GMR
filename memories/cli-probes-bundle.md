---
about:
  - console/cli/src/verbs/probes.rs#bundle
watch: [logic]
---

# A bundle ships only what is currently installed, and its index is rewritten, not copied

The working store under `store_dir` accumulates every artifact ever built
there, including ones for probes that are no longer declared. `bundle`
does not copy that store wholesale — it walks the current `recipes`,
resolves each one's currently-*installed* artifact, and copies only those
directories. A release tarball carrying every historical artifact would
grow forever and ship dead weight nobody asked for.

The bundle's `installed.json` is built fresh from `shipped` rather than copied
from the working store, for the same reason: the working index also names
artifacts that did not make it into this bundle (superseded builds, retired
probes), and copying it verbatim would let the bundle's index claim artifacts
the bundle does not actually contain.

It is written by opening an `Artifacts` over the bundle directory and calling
`install` per probe, **not** by a writer of its own. A second writer here would
carry its own copy of the schema string, one crate away from the `INSTALL_SCHEMA`
the reader checks — a version bump apart from a bundle no reader accepts, with
nothing standing between them to notice. See [[transport-artifacts-store]] for
who owns the format.

The built-in extractors are deliberately absent from this bundle — they
live in the binary itself (see [[extract-closure]]), so there is nothing
for `bundle` to ship for them.

## When this changes, ask

Does the new code copy the working store's install index or artifact
directory directly, instead of rebuilding a subset from `recipes` and
`shipped`? Copying directly would let the bundle claim artifacts it never
actually packaged.

Does it write `installed.json` itself again rather than going through
`Artifacts::install`? The file belongs to the battery that reads it.

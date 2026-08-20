---
about:
  - batteries/transport/src/shell/artifact.rs#InstallIndex
  - batteries/transport/src/shell/artifact.rs#Artifacts
  - batteries/transport/src/shell/artifact.rs#Resolved
watch: [sig, logic]
---

# The store is content-addressed; the install index only maps a name onto it

`Artifacts` lays artifacts out as `<root>/<version>/manifest.json` plus the
files the manifest names — the version, not the probe name, is the
directory. `InstallIndex` is the one place a probe name gets attached to a
version at all: the name travels between machines and across reinstalls,
the artifact directory never does, and the index is self-describing (it
carries its own `schema`) so no assembly has to thread a separate table
through to find it.

It is typed `BTreeMap<ProbeName, ProbeVersion>` rather than `BTreeMap<String,
String>`, which makes **this file's own schema the door**. Held as strings, a
truncated or hand-edited index hands back a version that is not one, and it
travels onward into `Derivation`, into the journal, and into a twelve-character
slice at some print site. Typed, the file refuses to decode and names the field
and the reason. `ProbeVersion` is a minted type precisely so that this works; see
[[core-newtype-classes]].

`Artifacts` is the only writer of this format — see [[cli-probes-bundle]] for the
one place that might otherwise grow a second.

Holding a `Resolved` is a proof, not just data: it is only constructed after
the manifest's own hash and every file it lists have been checked
byte-for-byte (see [[transport-artifacts-resolve]]). Nothing downstream
that receives a `Resolved` needs to re-verify it.

## When this changes, ask

Does the new code create a `Resolved` anywhere other than the end of
`Artifacts::resolve`, after all the byte checks? A `Resolved` built any
other way breaks the "holding one means it was verified" guarantee callers
rely on.

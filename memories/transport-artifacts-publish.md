---
about: batteries/transport/src/shell/artifact.rs#publish
watch: [sig, logic]
---

# The derivation is the publisher's claim; `publish` cannot compute it

`publish` takes `derivation: ProbeVersion` as a parameter rather than
deriving it from the files being published, because only the publisher —
the build that produced these bytes — has access to the sources the
derivation was earned from (source files, dependency closure, whatever
`derivation` is supposed to summarize). The artifact's *address* (the
manifest hash) is computed here from the published bytes; the *derivation*
it stands for is always handed in, never recomputed.

## When this changes, ask

Is there a way to derive `derivation` from what `publish` can see on its
own (the files in `from`)? If yes, the caller passing it in becomes
redundant, but if no, `publish` staying blind to it and trusting the
caller is the only honest option — computing a derivation from information
`publish` does not have would be a fabricated identity.

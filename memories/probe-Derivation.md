---
about: crates/gmr-core/src/probe.rs#Derivation
---

# An earned hash, not the bytes of a binary

`version` hashes **every input that can change the output** — the source files,
the versions of whatever parses them, the output contract. Decision 5.

**Not the bytes of a binary.** Bytes move with the platform and the compiler, and
an identity that says "the version moved although the behaviour did not" is noise
that nothing can filter out — every time someone changes machine, every anchor in
the repository reports "the probe changed", and everyone learns to ignore the
signal.

## When this changes, ask

Anything unrelated to "can the output change" got added to the version's inputs
(build time, machine name, target triple) → it is manufacturing that noise.
Conversely, an input that can change the output is missing from the hash → the
probe changed and nobody knows, which is far worse than noise.

## What earns a version is the transport's business

`Derivation` is only responsible for **carrying** the version and its
provability. How that version is computed — which source files are hashed, which
dependency versions are pinned — is decided by the concrete transport;
`coding-extract`'s `build.rs` is one example. The substrate does not dictate the
algorithm, only that it must close over its inputs.

Shipping a new probe version: derivation moves, declaration does not. Those two
have to be separable, which is why they are two fields in [[journal-Versions]].

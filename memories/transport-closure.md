---
about: batteries/transport/src/closure.rs#of_path
watch: [sig, logic]
---

# A directory's version has to cover path and bytes of every file, not just bytes

`of_path` hashes a file to its content, but hashes a directory to every
file under it by both path and bytes (`absorb` writes the relative path,
then the bytes, for each entry). Path has to be in there: moving the exact
same bytes to a different file inside the closure is still a real change to
what the closure means, and a hash of concatenated bytes alone would not
see it move.

`None` propagates from any unreadable entry rather than skipping it,
because a caller that cannot read part of a closure cannot honestly say
what version that closure is — reporting a version anyway would be
claiming an identity for bytes it never actually saw.

## When this changes, ask

Does the new traversal still fold in each file's path, not just its bytes?
And does an unreadable file still short-circuit to `None` rather than being
silently skipped — skipping it would let two closures that differ only in
an unreadable file collide on the same version.

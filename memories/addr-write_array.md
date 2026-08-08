---
about: crates/gmr-core/src/addr.rs#write_array
---

# The depth counter has to unwind on the error path too

`write_array` and `write_object` wrap their bodies in an immediately-invoked
closure `(|| { ... })()`, take the result, then do `self.depth -= 1`, and only
then return. That is not style — every `?` inside the body can return early, and
written straight as `self.depth += 1; ...?; self.depth -= 1;` one failed
canonicalization would raise the depth counter permanently, leaving every later
call to judge `MAX_CANONICAL_DEPTH` against the wrong baseline.

This only turns fatal once one canonicalizer instance is reused across several
`write` calls. `canonical_write` builds a fresh one each time, so today it cannot
be reached; make it reusable and it can.

## When this changes, ask

Does a new early return inside the body bypass `depth -= 1`? Replacing the
closure with plain `?`, or introducing a `return`, reopens this hole.

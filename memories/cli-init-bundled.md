---
about: console/cli/src/verbs/init.rs#bundled
watch: [logic]
---

# Canonicalizing the exe path is what makes bundled probes findable through a symlink

`bundled` ships probes next to the binary so users never have to build
them themselves. Every package manager installs a symlink in `bin/`
pointing at the real binary, and `current_exe()` can hand back that
symlink path rather than the real one — so `bundled` canonicalizes it
first. Without that, `exe.parent()` would be the symlink's directory
(often just `bin/`), and the sibling `probes/` directory that actually
sits next to the real binary would never be found.

## When this changes, ask

Does the new lookup still canonicalize before taking `.parent()`? Skipping
that step reopens the exact "probes/ next to the symlink, not the real
binary" failure this function was written to avoid.

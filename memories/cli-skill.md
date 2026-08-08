---
about: domains/coding/cli/src/skill.rs
watch: [sig]
---

# The skill doc ships inside the binary, like the extractors

`SKILL_MD` is loaded with `include_str!` at compile time rather than read
from disk at runtime, for the same reason the language extractors are
linked into the binary rather than shipped as separate files: distributing
it should need no change to the release pipeline — no extra asset to copy,
no path to get wrong on install. `gmr skill install` writes this string
out to `PROJECT_PATH` or `global_path()`; the source of truth for what gets
written is this compiled-in string, not a file the binary reads at
install time.

## When this changes, ask

Does the new path read `SKILL.md` from disk instead of the compiled-in
constant? That reopens exactly the packaging dependency `include_str!`
was chosen to avoid.

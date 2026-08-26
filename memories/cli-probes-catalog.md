---
about: domains/coding/cli/src/probes.rs#Catalog
watch: [sig, logic]
---

# `kind_of` decides how a name reaches a transport; nobody writes that by hand

`Catalog` is every probe this build can reach: the extractors linked into
the binary, plus whatever `probes.toml` declares as recipes or scripts.
`kind_of` computes which `Kind` (`builtin`/`script`/`http`/`shell`) a name
maps to by checking where the name is actually registered, rather than
reading a `kind` field a person wrote in the declaration — a name is either
linked in (and therefore `builtin`), declared under `[script.…]` or
`[http.…]`, or it falls through to `shell`. There is no option to get wrong
by hand, because nothing hand-writes this mapping in the first place.

`http` is the fourth and arrived the way this note says a fourth should: a
`[http.<name>]` table and one more structural branch, not a `kind = "http"`
line in the TOML for someone to contradict. See [[cli-fetched-facts]].

## When this changes, ask

Does a new probe source add a `kind` field a human fills in, instead of a
new branch `kind_of` can detect structurally? A hand-written kind can
disagree with where the probe actually lives; a detected one cannot.

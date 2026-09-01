---
about: console/cli/src/probes.rs#Catalog
watch: [sig, logic]
---

# `kind_of` decides how a name reaches a transport; nobody writes that by hand

`Catalog` is every probe this build can reach: the extractors linked into
the binary, plus whatever `probes.toml` declares as recipes or scripts.
`kind_of` computes which `Kind`
(`builtin`/`script`/`http`/`file`/`sql`/`shell`) a name maps to by checking
where the name is actually registered, rather than reading a `kind` field a
person wrote in the declaration — a name is either linked in (and therefore
`builtin`), declared under `[script.…]`, `[http.…]`, `[file.…]` or
`[sql.…]`, or it falls through to `shell`. There is no option to get wrong
by hand, because nothing hand-writes this mapping in the first place.

**A kind `kind_of` can name and `probes list` cannot show is a probe that
works while being invisible.** All three new families were missed there —
`gmr probes` went on listing four builtins and two older kinds while three
declared, working probes did not appear. Nothing connected `kind_of`
gaining a branch to that list gaining a loop, so a test now declares one
probe of every kind and asserts the listing shows each. That is the
connection; the prose above is not.

`http`, `file` and `sql` are the fourth, fifth and sixth, and all three
arrived the way this note says one should: a table of their own and one more
structural branch, never a `kind = "…"` line in the TOML for someone to
contradict. Their `obs` is not declared either — each reports a single field
its transport names, so `obs_of` builds it from that transport's constants
instead of trusting a `facts = [...]` list that is free to drift from what
actually comes back. See [[cli-fetched-facts]], [[transport-file]] and
[[transport-sql]].

## When this changes, ask

Does a new probe source add a `kind` field a human fills in, instead of a
new branch `kind_of` can detect structurally? A hand-written kind can
disagree with where the probe actually lives; a detected one cannot.

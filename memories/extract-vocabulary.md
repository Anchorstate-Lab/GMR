---
about:
  - domains/coding/extract/src/lib.rs#Vocabulary
  - domains/coding/extract/src/lib.rs#every_key_a_probe_declares_comes_back_from_a_real_run
watch: [sig, logic]
---

# `at` has two layers, and only one enters the semantic closure

The keys in `ITEMS` are **matchable**: they decide which candidate a coordinate
selects, so they live in the semantic closure and changing them should swap the
probe version. Keys in `at` but not in `ITEMS` (`form` · `surface` · `after`) are
merely **observable**, and nothing more.

The two layers cannot overlap. **A key that both takes part in the selection and
is read as an axis can never move** — the selected candidates are by definition
equal on it. That is how the `file` axis died.

`every_matchable_key_is_one_the_probe_declares` guards `ITEMS ⊆ at`: declaring a
matchable key the probe never emits makes every position written with it fail to
match, silently.

## Why `Vocabulary` is deliberately outside the closure

What it constrains is **which shapes can be fed to this probe**, not what the probe
**derives**. Changing one `reads` extension — or flipping a probe between
`Reads::Extensions(&[...])` and `Reads::Anything` — changes the result of no
observation whatsoever, and should not turn over every `fact_address` in the
repository. (`reads` used to be a plain `handles: &[&str]`, where an empty
list stood for "reads anything"; `Reads::Anything` made that state explicit
instead of implicit — see [[extract-routing]].)

The price is that it can drift apart from the candidate table: `Vocabulary` is
written in this file, while the candidates are built inside the closure. When they
part ways, some shape goes to read an `obs.at.<key>` that no candidate carries — a
rule fault, or worse, an axis that **can never move and nobody notices**.

So every declared key has to come back from a real run. That test walks the tied
candidates one at a time by `nth` and takes the union, because the report carries
the facts of the selected candidate only ([[survey-report]]). **The test pays
that cost, production does not.**

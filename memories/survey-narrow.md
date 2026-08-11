---
about:
  - batteries/survey/src/narrow.rs#narrow
  - batteries/survey/src/narrow.rs#touches
  - batteries/survey/src/narrow.rs#narrowing_never_changes_what_report_says
  - batteries/survey/src/narrow.rs#want
  - batteries/survey/src/narrow.rs#the_rounds_actually_reach_every_branch_they_claim_to
watch: [sig, logic]
---

# A candidate that matches nothing can never win, so it never has to exist

`report` picks the greatest hit vector and hands back everything tied with it.
Read the two branches at the top of it and the consequence falls out:

```
best is None, or every bit in it is false   ->  found:false, and nothing else is read
best has at least one true bit              ->  every tied candidate shares that vector,
                                                so every tied candidate hits >= 1 item
```

Everything `report` puts in its answer — `at`, `facts`, `candidates`, `roll`,
and whatever `nth` selects — comes out of `tied`. So the whole answer depends
only on the candidates that match **at least one** wanted `(key, value)` pair.
The rest cannot appear in the output under any coordinate, any `nth`, any
corpus. `found:false` is exactly "that set is empty".

That is what `narrow` returns, and it is why a query does not have to
materialise the repository to answer a question about one symbol. Today's
extractors derive every candidate in the tree and then rank them; the coordinate
only enters at the last step. `narrow` is the same computation with the losers
never built.

`narrowing_never_changes_what_report_says` is the property, run over five
thousand generated corpora and coordinates, plus the degenerate cases that are
easy to get wrong and impossible to notice: an empty corpus, a coordinate that
matches nothing, one that matches everything, an out-of-range `nth`, and a roll
over `MAX_BYTES`. The last two matter because both sides have to **refuse with
the same words** — an optimisation that changes an error message is still a
change to what the tool said.

### The generator had a blind spot the counters could not see

`the_rounds_actually_reach_every_branch_they_claim_to` counts how often each
branch was reached, because five thousand rounds prove nothing about a branch
they never entered. It was missing a shape of its own: `want` was built as
`KEYS.take(n)`, so it only ever emitted **prefixes**.

`wanted` does not work that way. It filters the probe's items against the
position, so a coordinate naming `file` and `name` but not `kind` yields a want
that skips an item and keeps a later one — and item order is precisely what
`best`'s lexicographic max turns on. A whole class of coordinate was outside the
five thousand rounds, and every counter was green.

`want` now picks each key independently and `gapped` counts the ones that skip.
Checked red by restoring the prefix generator: `gapped` is the only assertion
that fails, which is the point — the other four could not have told you.

`narrow` must stay a stable filter. `report` reads `nth` as an index into
`tied`, and `tied` inherits the order it was given. Reordering here renames
which object an anchor is about while nobody has touched the code — the failure
[[survey-walk]] exists to prevent.

## Why it is not in the semantic closure

`build.rs` hashes what can change an extractor's output. `narrow` provably
cannot: that is the whole content of the property above. Hashing it would mean
every future tuning of the union — an index, a different traversal, a cheaper
predicate — turns over every `fact_address` in every repository and asks
everyone to rebase, for an output that is identical byte for byte. That is the
"identity changed, behaviour did not" noise named in the architecture doc, and
no consumer can filter it.

The line is not "code in the battery is hashed". It is **what can change the
answer is hashed**: which files are eligible, when a cached entry is still
fresh, the sort key, the aggregation. Those change results. Storage, traversal
strategy, and this function do not.

Checked rather than assumed — the four extractor versions before and after this
file landed are identical.

## When this changes, ask

Does `narrow` still keep the candidates in the order it received them, and does
it still drop only candidates that hit **zero** wanted pairs? Dropping on any
stronger condition — "cannot beat the best so far", a cap, a cheaper
approximation — breaks the equivalence, and the property test is the only thing
standing between that and a query that quietly answers a different question.

---
about: batteries/survey/src/matching.rs#report
watch: [sig, logic]
---

# The report's field set is a contract, and both branches must agree

What `report()` emits comes in two layers, divided by **who it is about**:

- the nine in `REPORT` are **report-level** — about this act of selection, and not
  about any one candidate
- `PER_CANDIDATE` (`at` · `facts`) is the coordinate and the facts of **the one
  that was selected**; rules read them as `obs.at.x` / `obs.facts.x`

There is only one copy of `REPORT`. `domains/coding/cli/src/contract.rs`
`pub use`s it from here, and `unmet()` uses it to judge "does the probe emit the
field this rule reads". **It used to be two hand-written lists**, one here and one
copied into contract.rs, with a comment here that said in so many words "Not
enforced". Deleting the second beats checking both — with no second copy there is
nothing to drift apart.

## The two branches must have equal key sets

The early-exit path for `found: false` and the normal path have to emit **exactly
the same keys**;
`both_branches_report_the_same_keys_and_they_are_the_declared_ones` watches this.

The reason is not tidiness. A key that only one branch emits is `Absent` to any
rule reading it — and "which branch ran" is precisely what that rule is asking.
`roll` and `priority` once appeared only inside `found: true`, and the only reason
nothing blew up is that rule ordering puts `obs.exact == false` ahead of every
`Since` rule, so `obs.roll` never got a chance to be evaluated. **That is being
right by ordering, not by contract.**

## Priority order is not an implementation detail

Candidates are compared by taking their hit vectors lexicographically, so **the
order of the coordinate items is the priority**. Under `[name, file]`, a candidate
that matches only `name` beats one that matches only `file`. The order a probe
author writes `ITEMS` in declares "which field best preserves identity".
`priority` reports that order out instead of hiding it in a parameter.

An out-of-range `nth` is an error, not a clamp. Quietly substituting another
candidate means pointing the anchor at a different thing with nobody knowing.

## Why `matches` is gone

There used to be a `matches` as well, holding the `{at, facts}` of every tied
candidate. Once [[survey-roll]] took over its identity duties, all that was left
was bulk: one transition of `layer::gmr-core` came to 35,430 bytes of facts, of
which 34,892 were that field — 98%, and no criterion read it. The rest of it
(body hashes of the other tied members) was meaningless to the anchor anyway: an
anchor watches **one** thing.

The `MAX_BYTES` ceiling was originally set to catch that field. With it gone, an
equally wide coordinate produces a twentieth of the output; the ceiling stays, but
what it now guards is `roll`.

One cost: the test in `extract` that says "every declared `at` key must come back
from a real run" used to get every candidate at once from `matches`, and now runs
them one at a time by `nth`. **The test pays that cost, production does not** —
which is exactly the right place for the split.

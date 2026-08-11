---
about:
  - batteries/survey/src/narrow.rs#touches
  - batteries/survey/src/narrow.rs#narrow
  - domains/coding/extract/src/name.rs#collect
watch: [sig, logic]
---

# `Candidate` currently means two different things, and `narrow` is where that becomes a bug

`gather` returns `Vec<Candidate>`. For `ast-map`, `addr-map` and
`prose-map` those really are candidates: their `coord` keys are the probe's
declared `at` vocabulary, and `report` can select among them directly.

For `name-map` they are not. Since [[name-map-cache]], its `collect` emits one
object per identifier per file with `coord = {name, file}` and
`facts = {count, line}` — while `name-map` declares `at: ["name", "scope"]` and
`facts: ["occurrences", "file_count", "files", "first"]`. **`file` is not in
that probe's vocabulary at all.** These are partial tallies wearing a
`Candidate`'s type, and they only become candidates after `rolled` folds them.

So one function now returns two kinds of thing, and the type does not say
which.

## Why this is inert today and will not stay that way

The fragments never reach `report`, so nothing observes the confusion. But
`narrow` exists precisely to filter what `gather` returns, and `touches`
matches on any `coord` key it is handed. Point it at `name-map`'s fragments and
it will match them on `("file", …)` — a key that is not part of the coordinate
anyone anchored. Whether that happens to give the right answer for one probe is
not the standard; the standard is that a filter must know what it is filtering.

The original plan called for a separate `Fragment` type with an explicit
`Merge`, and reusing `Candidate` was a shortcut taken to land the cache without
making `Cache` generic over a per-probe fragment type. That is the real cost of
paying this off, and why it was not paid at the time: nothing benefited yet.

## The bill, measured

The paragraph above argued this from the types. It has since been run, over this
repository's own corpus, folding the whole fragment set against folding only the
union of it:

```
name-map   SAME   union     6/52425   {"name":"gather"}
           SAME   union     6/52425   {"name":"gather","scope":"batteries"}
           DIFFER union     0/52425   {"scope":"batteries/survey"}
              whole corpus: found=true  at={name:"A", scope:"batteries/survey"}  occurrences=2
              union only:   found=false
```

So the failure is not "the fold cannot use an index". It is narrower and worse:
**a key the fold derives cannot narrow the fold**. `scope` is computed from
`file` while folding and appears in no fragment's coordinate, so a coordinate
naming only `scope` selects nothing and the answer flips from `found:true` to
`found:false` — silently, and only for that shape of position.

Where the want does carry a fragment key, the fold over the union is the same
answer, and 6 rows out of 52425 is what it costs instead.

The rule that follows: **narrow on `want ∩ fragment coordinate keys`; where that
intersection is empty, read the whole generation.** The fallback still skips the
parse, which is what was expensive, so it is not the old cost coming back.

## The trigger

> Separate the two types **before `narrow` gets its first caller**, not before.

Right now `narrow` is called by nothing but its own property test, which is
deliberate — see the header of `narrow.rs`. The moment a `probe()` body calls
it, this note stops being debt and becomes a defect, because the same call site
will be handing it candidates from three probes and fragments from a fourth.

## When this changes, ask

Does `gather`'s return type distinguish "an answer this coordinate could
name" from "a partial tally about one file"? If not, does anything between the
scan and `report` filter, sort, count or truncate that vector? Those are the
operations that need to know the difference, and every one of them is silent
when it gets it wrong.

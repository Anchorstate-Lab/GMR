---
about: domains/coding/cli/src/memories.rs#superfluous
watch: [sig, logic]
---

# Whether the escape hatch was warranted is decided by walking the routing, not by convention

A note's normal form is `about: <coordinate>`; the long-hand
`anchors: - key/probe/position/shape` is the escape hatch. The problem is that "when
is the escape hatch really needed", written as a convention in a document, can only be
upheld by people remembering — and that is exactly what the owner called "should not
depend on an individual to maintain".

So the criterion is executable: **throw the `key` into `coord::route` as a coordinate
and see whether what comes out matches what was written by hand.** If it matches, and
there are no `rules` / `terminal` / non-default `params`, then the long-hand form said
nothing extra and one `about:` line is enough.

That makes the "four reasons for the escape hatch" not a list but this function's four
branches, mapping one-to-one onto README's four:

```
① rules or terminal written by hand      two reasons sharing one early return
② non-default params                     early return
③ coord::route returns Err               early — no probe eats this coordinate at all
④ routing produced something, but the probe or position differs from what was written
                                         the trailing boolean
```

The first three are early returns; the fourth is the function's return value —
**not four early returns**. The branch count matching the reason count is deliberate:
the list in the document was copied down from this function, not the other way round.

## When this changes, ask

`coord::route` gets more capable (the coordinate syntax gains `kind` or `member`, say)
→ this function will automatically judge more long-hand forms superfluous, and
`gmr doctor` will start reporting `long-hand`. **That is correct.** Do not add an
exemption here to quiet the report. If you want an exemption, state the reason, and the
reason has to be a branch.

A branch judging `false` can only make the lint under-report, never misfire — so it
would rather miss than convict wrongly. Add new branches by that rule.

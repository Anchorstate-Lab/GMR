---
about:
  - domains/coding/cli/src/render.rs#diagnosis
  - crates/gmr-runtime/src/read.rs#AnchorView
watch: [sig, logic]
---

# The answer to "why missing" has been sitting in the log all along, just never rendered

The bit vector says **which axis moved**; it cannot say **why**. And when `missing`
lights up, what the person actually wants to ask is "did the file go, or did the name
go".

That answer is in the probe's report, present at every observation, and simply never
printed:

```
found      = true          the file is there
exact      = false         but it is not an exact hit
matched    = ["file"]
missed     = ["heading"]
candidates = 7             there are 7 tied headings in that file
```

`doctrine::red-cards` lay like that for a whole stretch of history. `gmr check` printed
the single line `doctrine::red-cards   absent`, and there is nothing a person can do
with that line; now it prints one more —
"file matched, heading did not — this reading is about whichever of 7 others was
closest" — and the problem is clear on the spot.

## Why it goes through `AnchorView.facts` rather than `Observed`

`Observed` is "what happened in this one observation"; `AnchorView` is "what this
anchor looks like now". The diagnosis answers the latter — whoever opens `gmr status`
has not just run an observation. `read()` already took `sighting`, `derivation` and `fact_address` out
of `latest`, and `facts` is one more field on the same object that can be handed
over without explanation.

**The substrate does not interpret it.** `Facts` is passed through as-is and how to read
it is the domain's business — `diagnosis` recognises the schema `gmr.probe-coord.v1`,
and for any other probe (the script probes) it says nothing at all and returns `None`.
This is the same as decisions 3 and 11: the substrate can fetch a field, it does not
interpret what the field means.

## Why these are not stuffed into the state

The thought was tried and turned back by [[state-schema]]: one more field in the state
that takes part in no criterion and `should_still` judges every reading different,
writing a transition with no bit lit at all. **Fill a rendering gap with rendering.** The
observation is already in the log; taking it back out and reading it to a person needs
no help from the state.

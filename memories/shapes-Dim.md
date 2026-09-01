---
about:
  - console/cli/src/shapes.rs#Dim
  - console/cli/src/shapes.rs#Reads
  - console/cli/src/shapes.rs#CONTRACT
  - console/cli/src/shapes.rs#GONE
  - packs/coding/extract/src/ast.rs#RECIPE
---

# An axis is a fork in what you should go and do, not a kind of thing

Decision 6 cuts the system in two: which directions a probe emits is
**representation**, which directions an anchor watches is **attention**. `Dim` is
the whole of the attention side — so the answer to "how many axes should there be"
cannot be "how many kinds of change code can undergo". That is taxonomy, and it
subdivides forever.

There is one criterion: **if two changes would make a person do the same thing,
they are one axis; if they lead to different actions, they must be separate.** A
bit lighting up means "here is what you should now go and do".

`contract`'s eight come from that:

```
missing   the anchor points elsewhere now, or should be closed
name      it is called something else; every mention of the old name is now wrong
file      it lives somewhere else; confirm the move was intentional
kind      this is no longer the same sort of thing; rewrite the memory entirely
sig       go look at every caller
surface   the public surface changed; ask who is using it
logic     re-read this implementation, ask whether the contract still holds
place     confirm this move was intentional, within the file
```

The order is the priority (decision 10, first match wins) and decides only which
name `status` prints; the vector is complete, and `gmr status` shows every one.

## The second property: what kind of question this axis answers

`Dim` says whether an axis **should exist**; `Reads` says **what kind of question it
answers** — and that in turn decides when its bit falls. These have to be declared
separately, because they are not the same judgment.

```
Now    "is it this way right now"        recomputed from the reading each observation — missing
Since  "has it changed since you confirmed"   already happened, accumulates until accept — the other seven
```

**`Now` may clear on any observation, `Since` may not.** The difference is not
"does a human need to confirm it", it is whether the signal can be lost: a
condition that still holds re-announces itself on every observation, and even if
it drops it lights straight back up. But "it changed" is past tense — whoever
observes first consumes it, and in a deployment `pass` runs on a cadence and will
eat it before a human runs `check`. That is a silent failure path.

This criterion was written down after the fact. `missing` used to be the only
non-accumulating axis, but its non-accumulation was **an accident** — a hand-written
`false` in the generator, not derived from any declaration.
`what_an_axis_answers_decides_when_its_bit_falls` pins exactly this: a `Now` axis's
bit expression may not mention `state.v.<itself>`, and a `Since` axis's must.

**A `Now` axis holding says "this reading is not about my target"**, so its rule
keeps the last good reading, carries every `Since` bit through untouched, and comes
before all the `Since` rules. The ordering is a criterion, not a style; the reason
is in [[shapes-expand]].

## One measurement, two questions

`Reads::Since` carries a comparison operator, because a roster's `grew` and
`shrank` both read the one `count` slot and only differ in direction — "does the
new thing belong to this layer" and "who still depends on what left" are two
different actions, so by the criterion above they must be two axes. So one field
may be read by several axes, and `reading()` deduplicates by field.

There are only three operators, `!=` `>` `<`, and no more for a reason: `>` and `<`
mean something only on counts, and the count is the one ordered quantity in obs.

## Table and Vector used to be two systems

`Body::Table` was Phase A's transitional apparatus — a way to add vector shapes
without breaking the four old ones. It never got dismantled, so every capability
added afterwards (subscription · level triggering · accept · Reads) had to be
thought through twice, and whichever side was overlooked became **silently
disabled**: `layers.md` wanted to say "only tell me when the roster changed", but
roster was a Table, `watch` had no effect on it, and it could not say so.

**What was called a Table shape is not another kind of shape, it is somebody's
hand-written rules.** And hand-written rules are an **anchor-level** escape hatch
(decision 7), not one of the kinds of shape. Cramming those two things into one
enum is what created the dual track. Split apart: a built-in shape is always a set
of axes; an anchor with hand-written rules has no shape, `of()` returns `None`, and
its delivery falls back to edge triggering — a **known downgrade**, written down in
[[delivery-standing]].

## What this criterion has actually ruled out

**"Parameters changed" and "return type changed" should not be two axes.** Both
actions are "go look at every caller" — the same fork. Splitting them makes the
vector longer without changing what the reader does. `sig` collapsing into one bit
was once reported as a defect, and that was wrong — the real defect was in the
representation: `async` / `unsafe` / generic bounds equally mean "go look at every
caller", and they were not in `shape` at all. **Fix the representation, do not add
an axis.**

**`file` is this criterion applied twice, and the second time the answer flipped.**

It was deleted once, and rightly. ast-map listed its coordinate items with `file`
ahead of `name` and nothing else said what identity *was*, so a definition that
moved to another file was a different definition: `missing` fired and the event
then called `moved-file` could not happen. An axis that can never light up is not
"not implemented yet", it is a bit in the vector telling a lie.

What changed afterwards is the representation, not the criterion. `Recipe` now
declares `identity` apart from `items`, and ast-map's is
`["name", "callee", "member", "shape"]` — `file` deliberately outside it. A
definition that moves keeps its identity, so the report comes back `found` while
`exact` goes false, and `GONE`'s guard moved from `obs.exact == false` to
`obs.found == false` in the same change. "This moved to another file" became a
thing that can be observed, it is one action, and it earned the axis back under
the name `relocated`.

**This is the `sig` lesson arriving from the other side.** There the answer was:
do not add an axis, go fix the representation. Here the representation was fixed
for its own reasons and an axis that had been impossible became possible. The
criterion never moved; what it was being applied to did.

It is not `place`. `place` asks who you sit after *within* a file; `file` asks
which file. Two moves, two different things to go and confirm.

**`place` measures "who it sits after", not an absolute line number.** It used to be
the line number, and the consequence was measured: adding one `use` at the top of
`probe.rs` handed back four memories, and not one of those four definitions had
moved. An absolute line number is not a measure of "this definition moved", it is a
measure of "something above it got longer". The predecessor definition is: adding an
import changes nobody's predecessor; genuinely moving a function changes two (its
own, and that of whatever used to follow it), and both really are affected.

**`logic` still carries one known false positive**: renaming a local variable reads
as the implementation having changed.

This is **a decision, not a debt**. By the action criterion it should be split (a
rename requires no action), but a safe implementation needs real scope analysis, and
an approximation that gets shadowing or closures wrong trades the loudest axis's
false positive for a **false negative** — "the logic changed and nothing reported
it". Directionally it also agrees with the `place` decision: over-report, and let
the author judge whether it is reasonable. The price is occasionally re-reading a
memory, which is far cheaper than a miss.

To overturn it, first answer: how does the extracted normalization handle shadowing,
closures and macro bodies, and where does it fall back to when it cannot. Falling
back to **today's behaviour** (hashing the source text) is the safe answer; falling
back to "treat it as unchanged" is that false negative.

## When this changes, ask

Changing `identity` on an extractor's `Recipe` → which axes just became
possible, and which just became bits that can never light up? Identity decides
what counts as the same thing, so every axis measuring something outside it
depends on that line. `file` was deleted and restored by exactly this, and in
between, nothing observed the connection — this note was anchored to `Dim` and
`Reads`, and neither of them is where the answer lives.

Adding an axis → first: **when it lights up, what does the person have to do that is
different from all six existing ones?** If you cannot say → it belongs to an existing
axis, and what is missing is that axis's **representation**; go change the probe.

Removing an axis → ask: who reports that thing now? "Another axis will light up
incidentally" does not count — an incidentally lit bit has no name, and what `status`
prints then means something else.

Changing this structure (adding a field, changing how `obs` is written) → every rule
`expand` generates changes, and every contract anchor drifts on criteria at once. Go
through `accept --all --criteria`: one shape change is one decision, and one
rationale covers the batch. See [[shapes-expand]].

## Why the default subscription is everything

`watch` only decides whether a memory is handed back and what the exit code is; it
does not decide what gets recorded. Given that an axis's entry test is already "if it
lights up, you have to do something", every axis should report by default — a note
that wants less writes its own `watch: [...]`. The other way round (report little by
default, add what you want) makes the criterion say itself twice, in two places. The
two delivery paths are in [[delivery-standing]].

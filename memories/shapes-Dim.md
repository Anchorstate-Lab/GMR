---
about:
  - domains/coding/cli/src/shapes.rs#Dim
  - domains/coding/cli/src/shapes.rs#Reads
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

`contract`'s six come from that:

```
missing   the anchor points elsewhere now, or should be closed
kind      this is no longer the same sort of thing; rewrite the memory entirely
sig       go look at every caller
surface   the public surface changed; ask who is using it
logic     re-read this implementation, ask whether the contract still holds
place     confirm this move was intentional
```

The order is the priority (decision 10, first match wins) and decides only which
name `status` prints; the vector is complete, and `gmr status` shows every one.

## The second property: what kind of question this axis answers

`Dim` says whether an axis **should exist**; `Reads` says **what kind of question it
answers** — and that in turn decides when its bit falls. These have to be declared
separately, because they are not the same judgment.

```
Now    "is it this way right now"        recomputed from the reading each observation — missing
Since  "has it changed since you confirmed"   already happened, accumulates until accept — the other five
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

**`file` was deleted for this reason.** ast-map's `ITEMS` puts `file` ahead of
`name`, which declares that "a definition's identity is the thing called this in
this file". Under that identity, a function moving to another file is `missing`,
and the event `moved-file` cannot happen. An axis that can never light up is not
"not implemented yet", it is a bit in the vector telling a lie.

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

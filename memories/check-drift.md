---
about:
  - domains/coding/cli/src/verbs/check.rs#drifted
  - domains/coding/cli/src/verbs/mod.rs#swapped
watch: [sig, logic]
---

# check has to say when it does not hold itself

Two things can void the conclusions check printed above, with different causes and
different remedies, so they are two reports: `drifted` says the **criteria** do not
hold; `swapped` says the **readings** are not comparable. Both print last, each
carrying its own remedy verb.

# When the criteria are drifting, nothing check said above counts

`shapes::of()` requires `Transitions` to be exactly equal before it recognises a
shape. So **after any shape change and before `accept --criteria`**, every anchor in
the repository using that shape goes unrecognised — `delivers` receives `None`, falls
straight back to edge triggering, and the note's `watch:` stops applying entirely.

`gmr status` has always reported this. `gmr check` used to say nothing, and check is
the one that gets run daily.

This is the same illness as the `Body::Table`/`Vector` dual track: **a criterion
silently stops applying in some state**. Pulling the dual track apart did not kill the
ambiguity, it only moved it from an enum into an `Option` — `None` means both "this
anchor uses hand-written rules" (fall back to the edge) and "this anchor's criteria
drifted" (report it). With check reporting drift, `None` on that path is left meaning
only the first.

So it prints last, and says outright that the conclusions above cannot be trusted.
Printed earlier it would be buried by the "n of N handed a memory back" line that
follows — and during a drift that line is precisely the wrong one.

# A reading taken by a different instrument is not comparable to the baseline

`swapped` compares **who took the reading this anchor stands on**
(`view.derivation`) against **what this build resolves to now** (`rt.instrument`).
Unequal means the baseline was measured by a different instrument.

The consequence of not reporting it was measured: add one constant to
`batteries/survey/src/matching.rs` and rebuild, and all four extractors swap versions
(ast-map `1e1ac5ee`→`48db5084`, and the other three likewise). Every one of the
repository's 56 anchor baselines became incomparable at a stroke — and `check` exited
0 spotlessly, with `status` · `doctor` · `health` all saying nothing either. The only
thing that knew was `rebase --all`'s own selector, and **a selector is not a report**:
it speaks only once you have already decided to rebase.

This dimension can mislead in both directions: when the output did not change, the
whole repository is silent (the version moved, the behaviour did not); when the output
did change, every anchor reports `signature-changed` (which looks like somebody edited
the code). So this passage does not say "it changed", it says **this run cannot tell
the two apart**.

## Why it counts as check's job rather than something rebase shouts about

`rebase` demands a `--why` and seals the rationale; it is the verb that **acts**.
"Standing on an incomparable baseline" has to be known before a person acts, and the
thing run daily is `check`. Same as the section above: criteria or readings going
invalid is **a signal for a human to look at**, not a build failure, so it goes into
check and not into `gate.sh`.

One implementation, two callers — `swapped` lives in `verbs/mod.rs` rather than inside
check, because `rebase --all` selects the very same set of anchors. The reason is in
[[verbs-recapture]]: three copies, two of them copied correctly and one invented, is
the cause of that bug, not a symptom of it.

## The half that is not fixed: this fact gets eaten by an observation

`swapped` is derived from **the latest observation's** derivation, so whoever observes
first wipes it out. The only reason check can still report it is that it computes this
section **before** its own observe loop (right beside the `drifted` line) — reverse the
order and it goes silent permanently.

**But `pass` will not report it.** In a deployment `pass` runs on a cadence, and the
moment it observes it writes the new derivation in, so a human running `check`
afterwards sees nothing. This is exactly what [[shapes-Dim]] wrote down for `Since`
axes: "it changed" is past tense, and whoever observes first consumes it.

The real fix is to record this where an observation cannot consume it — a bit, or a log
entry — rather than deriving it from the latest reading. That is **a change of criteria
in the substrate** and needs the owner's call (decisions 5 and 7), so this version fixed
the report and not the retention. Until then: **run `check` after a rebuild, before you
let `pass` go.**

**This does not go into `gate.sh`.** It has to read `.anchor/state/memory.db`, and half
the reason gate.sh's six checks have never drifted is that it touches no anchor —
turning CI red because somebody has not run `accept` locally converts "a signal for a
human" into a build failure. gate.sh checks identities of the source tree; this one
checks whether one particular repository's anchor store lines up with its own notes,
which is a different object.

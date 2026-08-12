---
about:
  - domains/coding/cli/src/verbs/sync.rs#Standing
  - domains/coding/cli/src/verbs/sync.rs#Audit
  - domains/coding/cli/src/verbs/sync.rs#standing
  - domains/coding/cli/src/verbs/sync.rs#audit
  - domains/coding/cli/src/verbs/check.rs#criteria
  - domains/coding/cli/src/verbs/doctor.rs#undeclared
  - domains/coding/cli/src/verbs/mod.rs#swapped
watch: [sig, logic]
---

# check has to say when it does not hold itself

Four things can void the conclusions check printed above, with different causes and
different remedies, so they are four reports. Three come out of one classification
(`sync::standing`, batched over a view set by `sync::audit`) and one is measured
separately:

| | says | remedy |
|---|---|---|
| `drifted` | the **criteria** do not hold | `accept --criteria` |
| `unreadable` | a note names this coordinate and this build could not turn it into a declaration | fix the coordinate |
| `undeclared` | a memory is bound and **no note declares it at all** | `close`, or write the note again |
| `swapped` | the **readings** are not comparable | `rebase` |

All print last, each carrying its own remedy verb.

## An anchor whose note is gone is not an anchor that agrees

`Standing::{Drifted, Unreadable, Undeclared}` are the three ways `sync::standing`
can fail to find a clean match for a key that is in the journal. Two of them are
loud. The third used to be a bare `continue` in a copy of this loop that lived
inside `check.rs` alone: delete the note, and the anchor kept its journal, kept
being observed, kept answering — while `differs` was never called on it, because
there was nothing left to compare against. Deleting a note is how an anchor
stops being supervised without anybody closing it, and until this report existed
it was the quietest way to do it.

`barren` is not the same thing and does not cover it: barren is an anchor with
**no memory bound at all**. This one has a memory, bound at a version git can
still fetch — which is exactly why it keeps working and why nothing complained.
The predicate is therefore "has memories, and no declaration", and `gmr anchor`
cannot trip it because it writes the note it declares from.

`check`, `doctor`, `status` and `accept` each used to run this same "declared vs.
live" walk as an independent, hand-written copy — and the fix for `undeclared`
above had to be written once for `check.rs` and, in a separate commit, a second
time for `doctor.rs`, because they were never the same function to begin with. A
third divergence (an unrouted note reported by `doctor` as deleted rather than
unreadable) landed and was fixed the same way, again separately. `sync::standing`
classifies one anchor; `sync::audit` batches it over a view set. `check` still
fetches its views with a per-key `rt.read` (it may be given a subset of keys),
`doctor`/`status` pass in the `live` slice they already hold from `rt.read_all`,
and `accept` calls `standing` directly for a single anchor's `Pending` facets —
but all four now go through the one classification, so a fourth divergence of
the same kind cannot happen silently again.

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

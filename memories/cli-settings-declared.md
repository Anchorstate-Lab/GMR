---
about:
  - domains/coding/cli/src/settings.rs#Declared
  - domains/coding/cli/src/settings.rs#at_open
  - domains/coding/cli/src/settings.rs#overlaid
  - domains/coding/cli/src/settings.rs#a_declaration_that_says_nothing_changes_nothing
  - domains/coding/cli/src/verbs/sync.rs#a_knob_the_toml_does_not_name_arrives_unsaid_rather_than_defaulted
watch: [sig, logic]
---

# A partial statement has to be a partial type, or it overwrites what it cannot say

`RunSettings` has four fields, and `sync` reconciles a declaration against what
an anchor is running. Building a **complete** `RunSettings` out of an **incomplete** declaration and
then comparing by equality resets, on every sync, every knob the declaration had
no way to mention — and to a value nobody wrote. It is unrecoverable in the
ordinary case, because the flags that set these live only on `open` and `open`
refuses an anchor that is already open.

`Declared` is all `Option`, so a declaration that says nothing is *shaped* like a
declaration that says nothing.

## Two readings, and they only differ once an anchor is running

`at_open` is for a new anchor: there is nothing to overwrite, so unsaid means the
deployment default. `overlaid` is for one already running: unsaid means
unchanged, and it returns `None` when the declaration moves nothing, so `sync`
reports a `resettled` only when something was resettled.

They are one struct read under two questions, and only the second can destroy
something. One function serving both is the shape this exists instead of.

## Nothing became unreachable

Unsaid meaning unchanged raises the obvious worry: how do you put a knob back to
the deployment default? By naming that default. `cadence_secs: None` in the
store and the deployment's own cadence written out behave identically — see
[[store-settings]] on what `None` means there — so every reachable state is
still reachable, it just has to be stated rather than implied by a deletion.
Stating it is the point: a declaration changes what it names.

## Four knobs, one vocabulary, both grids

`AnchorDecl` and a note's long-hand `Spec` both `#[serde(flatten)]` the same
`Declared`, so `.anchor/anchors.toml` and YAML frontmatter can say exactly the
same things. `flatten` is the one step that could quietly turn *absent* back
into `false`/`None` — it is the whole difference between overlaying and
replacing — so both grids have a test standing on it, one through `toml` and one
through the note scan.

`about:` stays saying nothing about how an anchor runs, and that is now correct
rather than destructive. It is one line naming a coordinate; see
[[cli-notes-source]] for why that line is deliberately not a place to put
operating knobs.

## The same rule, one door over

`Routed` carries `params`, so routing a coordinate has one answer whichever door
asks — `gmr open` and a note's `about:` produce the same `ProbeRef` rather than
two that differ by a default nobody chose, which would leave the anchor in
`criteria_drifted (probe)` with nothing to reconcile. `--params` is an
`Option<String>` for the same reason the knobs are: unstated has to stay
distinguishable from stated-as-empty, here in the half of the declaration that
*is* sealed criteria.

## When this changes, ask

Does a new field on `RunSettings` arrive on `Declared` too? A knob only `open`
can set is this bug again, and it will present as sync quietly undoing somebody's
tuning rather than as anything that looks like a bug. `digests`
([[anchor-recorded]]) was the fourth and arrived through this door because this
note was standing at it.

Does anything build a whole `RunSettings` from a declaration in order to compare
it? That comparison is only sound while the declaration can express every field,
which is a property of two types staying in step — not something either one
enforces on the other.

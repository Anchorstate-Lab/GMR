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

`RunSettings` has three fields. `AnchorDecl` could express two of them, and a
note could express none — `from_about` and `from_spec` both wrote
`retain_full: false, cadence_secs: None` in, and there was no `budget_ms` field
to write at all. Then `sync` reconciled by whole-struct equality:

```rust
if rt.settings_for(&key).await? != decl.settings() { Resettle(decl.settings()) }
```

`decl.settings()` built a **complete** `RunSettings` out of an **incomplete**
declaration, so a knob the declaration had no way to mention was reset, on every
sync, to a value nobody had written. Measured: opening with `--retain-full
--cadence-secs 900 --budget-ms 7000` and then adding one line of `about:`
frontmatter took `full | 900 | 7000` to `tick | NULL | NULL` — and it could not
be put back, because those three flags exist only on `open` and `open` refuses
an anchor that is already open.

The type was lying. `Declared` is all `Option`, so a declaration that says
nothing is *shaped* like a declaration that says nothing.

## Two readings, and they only differ once an anchor is running

`at_open` is for a new anchor: there is nothing to overwrite, so unsaid means
the deployment default. `overlaid` is for one already running: unsaid means
unchanged, and it returns `None` when the declaration moves nothing, so `sync`
stops reporting a `resettled` that resettles nothing.

Collapsing the two into one function is the shape that just came out. They are
the same struct read under two different questions, and only the second one can
destroy something.

## Nothing became unreachable

Unsaid meaning unchanged raises the obvious worry: how do you put a knob back to
the deployment default? By naming that default. `cadence_secs: None` in the
store and the deployment's own cadence written out behave identically — see
[[store-settings]] on what `None` means there — so every reachable state is
still reachable, it just has to be stated rather than implied by a deletion.
Stating it is the point: a declaration changes what it names.

## Three knobs, one vocabulary, both grids

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

## The same mistake, one door over

A coordinate opened by hand and the same coordinate declared by a note used to
produce **different** `ProbeRef`s — `--params` defaulted to the string `"{}"`
while `from_about` wrote `{"root": "."}` — so that anchor sat in
`criteria_drifted (probe)` forever. `Routed` now carries `params`, so routing a
coordinate has one answer, and `--params` is an `Option<String>`: unstated is
unstated, not `"{}"`. Same lesson as the knobs, in the half of the declaration
that *is* sealed criteria.

## When this changes, ask

Does a new field on `RunSettings` arrive on `Declared` too? A fourth knob only
`open` can set is this bug again, and it will present as sync quietly undoing
somebody's tuning rather than as anything that looks like a bug.

Does anything build a whole `RunSettings` from a declaration in order to compare
it? That comparison is only sound when the declaration can express every field,
and the two have drifted apart once already.

---
about:
  - domains/coding/cli/src/verbs/sync.rs#run
  - domains/coding/cli/src/verbs/sync.rs#synced
  - domains/coding/cli/src/verbs/sync.rs#tell
  - domains/coding/cli/src/memories.rs#Fault
watch: [logic]
---

# Sync tells apart three different kinds of "something changed"

For each already-open anchor, `run` checks three independent things and
reports them under three different labels, because they call for three
different responses:

- **Criteria drift** (`differs`, see [[cli-sync-differs]]): the
  declaration itself changed — probe, rules, or terminal set. This needs a
  human decision (`revise`), which is why sync only reports it and never
  applies it.
- **Instrument swap**: the declaration is identical, but `rt.instrument`
  now resolves the same probe name to a different derivation than the
  anchor's last recorded one (see [[runtime-instrument]]). That is not a
  criteria change — it is a baseline taken with a different ruler — and
  only a person can say whether the old baseline still counts (`gmr
  rebase`). **Closed anchors are skipped**: finishing is irreversible, so
  `rebase` refuses them, and reporting them here pointed the reader at a
  command that would tell them the same list was empty. A warning naming a
  remedy that does not apply is worse than no warning — it is how a list
  people are supposed to act on becomes one they learn to scroll past.
- **Resettling**: the declaration *names* a knob whose value differs from the
  one this anchor is running with. These are not criteria at all (see
  [[anchor-RunSettings]]), so sync just applies them directly with
  `set_settings`, no human decision required.

  "Names" is load-bearing: comparing whole-struct against a `RunSettings` built
  from the declaration would reset every knob the declaration cannot mention, and
  report it as `resettled` — a verb undoing somebody's tuning while looking like
  a verb doing its job. `Declared::overlaid` is what keeps it to what was named;
  [[cli-settings-declared]] is why.

For anchors not yet open, `check_contract` runs before the `dry_run`
branch returns early — a rule reading an obs field its probe never emits
is refused whether or not this particular run would actually open
anything, so `--dry-run` still surfaces the same contract errors a real
sync would hit.

## Two phases, and the fault weight it shares with `doctor`

`run` resolves every declaration and every binding into a `Vec<Step>` before
it performs the first write. Anything that can fail — routing a coordinate,
building a probe, reading the contract, versioning a reference — happens in
the first phase, so a failure leaves the repository exactly as it was rather
than partly synced. `--dry-run` is now simply "build the plan and stop",
which is what it always claimed to be.

The exit code counts `breaks()` faults, not only `blocks()` ones. It used to
count only the latter, and `doctor` counted the former — so a repository
where every note failed to declare anything was red under one verb and green
under the other. Measured: 147 notes in, zero anchors out, `sync` exit 0.
`doctor` was the only thing that objected, and CI does not run `doctor`.

Two verbs disagreeing about what counts as broken is worse than either
answer being wrong, because the quieter one is the one people automate.

## A lint that cannot see the answer is asked again where it can

`bare-key` says a key "binds without declaring; nothing else in this repo
declares anchors". `claims_of` cannot check that second clause — it has one
note and a probe catalog, not `anchors.toml` and not the open anchors. So
the scan reports every bare key, and `run` calls `Scanned::accounted_for`
with the keys `anchors.toml` declares and the keys already open.

Raising the exit code to `breaks()` is what made this matter. Before, the
false positive was advisory noise; after, a repository doing exactly what
the front door documents — a hand-written script probe declared in
`anchors.toml`, a note binding to it by bare key — exited 1 from `sync` with
the anchor opened and the note bound, on a lint whose own sentence was
false. `doctor` resolves it the same way for the same reason.

## The act and the telling are two functions, because one verb composes this one

`synced` does the work and hands back a `Synced`; `tell` renders it; `run` is
the two together, which is all `main` ever needs. `anchor` needs the first
without the second — it is the only verb in this CLI that invokes another,
and it folds this report into its own single answer rather than letting a
second one land on the same stream ([[cli-anchor-declares]]).

That `anchor` reaches for it at all is worth naming: declaring **one**
coordinate runs the **whole repository's** reconciliation, because opening a
declaration is what `sync` does and it does it for every declaration at once.
So the exit code `anchor` returns is this report's, about the repository, and
not about the record it just bound. Both answers are in its output for that
reason.

`Synced.broken` owns its faults rather than borrowing them from the scan: a
report is a snapshot of what happened, and one that borrows cannot outlive
the run it describes.

**`broken` in `--json` is `Fault`'s own field set, minus `weight`.** That is
why `Fault` is anchored here from another module: a field added to a lint
value now appears in a contract agents read, and the only thing standing
between the two is a `#[serde(skip)]` somebody has to remember. Nothing else
in this repository observes that, so this note is what observes it — a change
to `Fault` should send the reader here to ask whether the new field belongs
in the report.

`weight` is the one that must stay out: it is how this repository decides
red, and `blocks()` / `breaks()` is a judgement callers are meant to read off
the exit code, not re-derive from a number in the body.

## When this changes, ask

Does a new kind of "changed since last sync" get reported under drift,
swap, or resettling — or does it need a fourth category? Conflating any
two of these would apply something that needed a human decision, or
demand a decision for something sync could safely apply on its own.

Does a new failure condition get its own weight, seen by one verb and not
the other? The weights are shared on purpose; a fault only one verb reacts
to is a fault that whichever verb runs in CI decides to ignore.

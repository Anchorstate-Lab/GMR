---
about: domains/coding/cli/src/verbs/sync.rs#run
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
  rebase`).
- **Resettling**: `retain`/`cadence_secs` differ from the declaration.
  These are not criteria at all (see [[anchor-RunSettings]]), so sync just
  applies them directly with `set_settings`, no human decision required.

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

## When this changes, ask

Does a new kind of "changed since last sync" get reported under drift,
swap, or resettling — or does it need a fourth category? Conflating any
two of these would apply something that needed a human decision, or
demand a decision for something sync could safely apply on its own.

Does a new failure condition get its own weight, seen by one verb and not
the other? The weights are shared on purpose; a fault only one verb reacts
to is a fault that whichever verb runs in CI decides to ignore.

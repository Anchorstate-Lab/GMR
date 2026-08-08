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

## When this changes, ask

Does a new kind of "changed since last sync" get reported under drift,
swap, or resettling — or does it need a fourth category? Conflating any
two of these would apply something that needed a human decision, or
demand a decision for something sync could safely apply on its own.

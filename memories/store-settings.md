---
about:
  - crates/gmr-store/src/settings.rs#Settings
  - crates/gmr-store/src/settings.rs#get
  - crates/gmr-store/tests/durable.rs#run_settings_are_meant_to_be_overwritten
watch: [sig, logic]
---

# Storage for operating knobs, not for anything a judgment depends on

`Settings` stores `RunSettings` mutably, with no append-only history and no
sealing — that is the correct storage shape precisely because
`RunSettings`'s fields are operating knobs, not criteria; see
[[anchor-RunSettings]] for why they were kept out of the sealed `Anchor` in
the first place. Rewriting one here does not reopen anything a past
transition judged.

`get` returning `None` means nothing was ever explicitly set for that
anchor, not "observe nothing" — the caller is expected to fall back to the
deployment's own default in that case, the same way `RunSettings`'s own
`cadence_secs: None` means "use the deployment's default pace" rather than
"do not observe".

`run_settings_are_meant_to_be_overwritten` is the mirror of the durability
tests around it (`sealed_records_refuse_to_be_rewritten`, the append-only
journal test): where those prove a blocked write, this one proves the
opposite — a second `put` for the same anchor must succeed and simply
replace the first, with no trigger standing in the way.

## When this changes, ask

Would the new field, if it existed on `RunSettings`, change the outcome of
any transition? If yes, it does not belong behind this trait at all — see
[[anchor-RunSettings]]'s entry test.

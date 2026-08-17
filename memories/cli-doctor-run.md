---
about:
  - domains/coding/cli/src/verbs/doctor.rs#versioning_is_broken
  - domains/coding/cli/src/verbs/doctor.rs#run
  - domains/coding/cli/src/verbs/doctor.rs#Verdict
  - domains/coding/cli/src/verbs/doctor.rs#theirs_to_fix
  - domains/coding/cli/src/verbs/doctor.rs#grounds
watch: [sig, logic]
---

# `doctor` has one definition per fact, and a strict line between "broken" and "worth noting"

`versioning_is_broken` checks for `.git` because git is how notes are
versioned here — outside a git repository, `bind` still succeeds (it can
still stamp a content hash), but fetching a note back at the exact version
it was bound at cannot work, since that requires history git alone
provides here.

`run` reuses `corpus_health`'s `barren_anchors` for the `barren` list
rather than running a second `memories.is_empty()` scan over the same
`AnchorView`s doctor already has in hand — one definition of "unbound"
instead of two that could quietly drift apart from each other.

## The exit code is decided by who can fix it, not by how bad it sounds

`Verdict` is one `bool` per condition that turns a run red, and
`theirs_to_fix` is the whole rule: **can the person holding this repository
make this go away by doing something here?** `stranded`, a provider that
failed to register, breaking note lints, `undeclared`, a record the store
says is `gone`, a binding through a provider this binary lacks, and a
stale installed SKILL.md all pass that test — a rebuild, an unbind, an
edit, a re-init.

A store that would not answer does not, and that is why `unreachable` is
**not a field on `Verdict` at all**. Somebody else's service having a bad
minute, or a total budget running out mid-walk (see [[content-budget]]),
is not something a build can be failed over: the owner cannot act on it,
so a red build only teaches them to stop reading the colour. The same goes
for the count of rewritten records that cannot show their before —
[[runtime-grounding]]'s degraded but honest answer, worth printing and not
worth failing on.

`absent`/`barren`/`unseen` stay informational for the older version of the
same reason: they are normal states (criteria written before the code
exists, a probe temporarily failing), not something misconfigured.

Asking "who can fix it" also makes a *new* condition mechanically
classifiable, which the previous list of four could not be — it was four
names with no stated principle joining them, so the fifth was going to be
argued about rather than derived.

`undeclared` is computed by `doctor.rs#undeclared`, which now calls the one
classifier both `doctor` and `check.rs#criteria` share — see
[[check-drift]] for `sync::standing`/`sync::audit`. What doctor still keeps
for itself is *which views it hands in*: it already has `live: &[AnchorView]`
from `rt.read_all`, so it passes that slice straight to `sync::audit`, where
`check.rs#criteria` does an async `rt.read` per key (it may be asked about a
subset of keys `read_all` would not give it). The classification — is this
key drifted, unreadable, or undeclared — is one function either way, so the
two callers cannot again disagree on the answer, only on how many anchors
they're asking about.

`run` also resolves the `bare-key` lint before weighing it, with the keys
`anchors.toml` declares and the keys already open — the check `claims_of`
cannot make from inside one note. Both verbs do this, because both gate on
`breaks()`; see [[cli-sync-run]] for what the unresolved version cost.

## When this changes, ask

Does the new signal answer yes to "someone holding this repository can make
this go away by doing something here"? If not, it prints and does not count
— and if a field for it appears on `Verdict`, that is the claim being made,
whether or not anyone meant to make it.

Does a new count reuse an existing source of truth (like `corpus_health`)
instead of re-deriving the same fact a second way?

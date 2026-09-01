---
about:
  - console/cli/src/verbs/doctor.rs#versioning_is_broken
  - console/cli/src/verbs/doctor.rs#run
  - console/cli/src/verbs/doctor.rs#Verdict
  - console/cli/src/verbs/doctor.rs#theirs_to_fix
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

**It keeps no definition of its own for records either.** Classifying groundings
here would be a second answer beside `edges`', over a different set of anchors,
and the two would be free to disagree while this one holds the exit code. The
classification lives in `CorpusHealth`, computed over `Corpus::views`, and `run`
reads `Footing` off it. `live` is used only for the two questions it is the
right slice for: `absent` and `stranded`. See [[runtime-corpus]].

`unseen` used to be a third, and it was this section's own rule being broken in
plain sight: `live.filter(|v| v.faltering.is_some())` is a second definition of
blindness beside `Warrant`'s. It now reads `CorpusHealth.knowings`, and the union
it prints is the same set it always was — after `Open` an anchor always has a
`last_sighting` and a `derivation`, so `faltering: None` is exactly `Seen`. What
changed is that there is one definition instead of two that happened to agree,
and that the three classes are now printed apart: `Unreachable` is somebody
else's service, `Unusable` is whoever writes the probe, `Unevaluable` is whoever
wrote the rules. Three different people, three lines — [[render-warrant]].

The fact-side lines beside them (`moved`, `quiet`, `incomparable`, `absent gnd`,
`no ground`, `undated`, from `CorpusHealth.holdings`) print counts and are on no
`Verdict` field. Ground moving is `check`'s sentence and `check` exits on it; two
verbs going red for one fact is the drifting second copy in exit-code form.
Without them `doctor` could name the records the store had lost and not one that
stood on ground that had moved.

`moved` and `quiet` are the same split `check` makes and for the same reason
([[cli-observe-vs-check]]): `Holding::Moved` says the ground moved, and the
note's `watch:` says whether anyone asked. Only the first count is something
`check` will hand back — pointing a reader at `check` for the second sends them
to a verb built to stay silent about it. The split is `Subscriptions::delivers`,
the one predicate that answers it, rather than a second reading of the axes here.

`quiet` does not fall on its own. A record whose ground moved on an axis nobody
watches stays counted until a fresh dated assertion re-dates it
([[runtime-warrant]]), and re-dating it asserts somebody read it. So the number
standing still is the corpus being honest, not a queue going unserviced.

## What a declared store can do is printed, and never weighed

The `provider` lines describe every store `.anchor/providers.toml` declares
(see [[cli-providers-recipe]]) and are not a `Verdict` field, because a
description is not a condition. A store that cannot be listed is not broken,
and nothing here can be done about it — but a reader who learns it by
watching a command fail reads every such failure as the store being down.
Saying it at assembly is the whole point, and colouring the run for it would
undo it.

## The exit code is decided by who can fix it, not by how bad it sounds

`Verdict` is one `bool` per condition that turns a run red, and
`theirs_to_fix` is the whole rule: **can the person holding this repository
make this go away by doing something here?** `stranded`, a provider that
failed to register, breaking note lints, `undeclared`, a record the store
says is `gone`, a binding through a provider this binary lacks, a stale
installed SKILL.md, and a record left `unsupervised` all pass that test — a
rebuild, an unbind, an edit, a re-init, a supersede.

`unsupervised` is a record every one of whose anchors has closed, or which names
an anchor nobody ever opened: a note still claiming something about the code with
nothing observing it — the exact state this tool exists to make visible — and the
owner can act on it three ways. [[runtime-corpus]] has the mechanism.

`chain_broken` passes the same test and is the sharpest case of it. The journal
is this repository's own file, append-only by trigger, so a link that no longer
covers its row means something got past that trigger or edited the file
underneath ([[store-journal-chain]]). Nobody else can go and look. It prints
above the anchor counts rather than among them because it is not a fact *about*
the corpus — it is the reason to distrust every fact printed after it.

Running it here and not in `check` is deliberate: it costs about a second on
this repository's 58k entries against `doctor`'s five, and `check` is the verb
that runs constantly. Behind a flag it would be a tamper check nobody runs.

A store that would not answer does not, and that is why `unreachable` is
**not a field on `Verdict` at all**. Nor is `never_asked`, which is the
same answer with a different cause: the total content budget ran out before
that record's turn, so nothing was asked and nothing is known. It gets its
own line rather than sharing `unreachable`'s, because the reader needs to
know that **what doctor printed above it is a partial view** — `never_asked`
next to `bound` is the only thing on the page that says how much of the
repository this run actually looked at. Raising `--content-total-ms` is the
remedy, and it is not something a red build would have taught anyone.

Somebody else's service having a bad minute, or a total budget running out
mid-walk (see [[content-budget]]), is not something a build can be failed
over: the owner cannot act on it, so a red build only teaches them to stop
reading the colour. The same goes
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

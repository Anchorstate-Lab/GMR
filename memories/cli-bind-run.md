---
about: domains/coding/cli/src/verbs/bind.rs#run
watch: [logic]
---

# The git-only pre-check is gone, because the generic answer finally says as much as it did

`run` used to test `root.join(&path).exists()` when `provider == "git"`,
purely to produce a better sentence: *"`<path>` is not in this repository"*
instead of the generic *"no content provider could version `<path>`"*. The
special case earned its keep only because that generic sentence was vague —
and it was vague because `current_version` returned `Ok(None)` both when no
such provider was registered and when a registered provider had no such
record (see [[runtime-current-version]]).

Once those two became distinguishable — `Err(NoProvider)` for the first,
`Ok(None)` for the second — the generic path could say *"`git` has no
record `<path>`"*, which is the same fact the pre-check was reaching for,
for every provider rather than one. So the branch was deleted rather than
generalised: there was nothing left for it to add.

`current_version` is called here rather than reaching for the git backend
directly, because it is the same function `read` and `edges` use to decide
"what version does this reference have right now". A second, separate
lookup here could drift from what those verbs report for the same
reference.

`root` left the signature with the check, since nothing else in this verb
needed it. That is the visible half of the change; the reason is above it.

## When this changes, ask

Does a new provider get its own existence pre-check bolted on here? It
should not, and now it has no precedent to point at: the provider's own
answer through `current_version` is the generic "does not exist here", and
the one case that used to be special stopped being special the moment that
answer got specific enough.

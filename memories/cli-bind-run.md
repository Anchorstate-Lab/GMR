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

`root` is not in the signature: it was here for that check and nothing else in
this verb needs it.

## A detach does not stand on the record, it stands on the tie

Binding to anchors needs the record's current version, because the binding states
what the record said when it was tied there. **Detaching does not.** It removes
the tie, and the version column on that row is view metadata about a tie that is
ending — so it carries the version the binding already held.

Asking the provider for a current version before a detach makes the one state
that most needs an unbind the one where it is refused: a record the store reports
as `gone` is exactly what `doctor` tells the owner to restore *or detach*, and
`current_version` answers `Ok(None)` for it. A remedy a verb names and then
declines to perform is worse than no remedy named, because the reader stops
believing the line.

Detaching a reference nothing is bound to is still refused — there is no tie to
end, and appending a row saying otherwise would put a binding in the table that
never existed.

## When this changes, ask

Does a new provider get its own existence pre-check bolted on here? It should
not: the provider's own answer through `current_version` is the generic "does not
exist here", and it is specific enough that no store needs a special case.

Does a new flag start requiring the record to be fetchable? Ask what the flag
actually states. Anything that ends a tie can be stated without the store
answering at all, and requiring an answer means the flag stops working precisely
when the store has bad news — which is when it is wanted.

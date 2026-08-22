---
about: domains/coding/cli/src/verbs/bind.rs#run
watch: [logic]
---

# The verb states a tie, and only tying needs the record to be there

Binding to anchors needs the record's current version, because the binding
states what the record said when it was tied there. `current_version`
supplies it — the same function `read` and `edges` use, so no second lookup
can drift from what those verbs report for the same reference.

A registered provider holding no such record answers `Ok(None)` and this
verb names it; a provider nobody registered is `Err(NoProvider)`. Neither
needs a per-store existence check: the generic answer is already the
specific one (see [[runtime-current-version]]). `root` is not in the
signature because nothing here needs it.

## `--detach` revokes, and never fetches

Detaching writes a revocation naming every live tag the record holds, one
per anchor, each recorded at that anchor. A revocation is not an assertion,
so it carries no version and asks the store nothing.

That is what lets it work in the state that most needs it: a record the
store reports as `gone` is exactly what `doctor` tells the owner to restore
*or detach*. A remedy a verb names and then declines to perform is worse
than no remedy named, because the reader stops believing the line.

Detaching a reference nothing is bound to is refused — there is no tie to
end.

## What it records about itself

A bind typed here is `Source::Adjudicated`: a person named the reference and
named the anchors. `sync` records `Source::Derived` for the same relation
reached another way. The two are told apart by kind of act, not by who ran
the command — see [[store-binding-record]].

## When this changes, ask

Does a new provider get its own existence pre-check bolted on here? The
provider's own answer through `current_version` is the generic "not here",
and it is specific enough that no store needs a special case.

Does a new flag start requiring the record to be fetchable? Ask what the
flag states. Anything that ends a tie can be stated without the store
answering at all, and requiring an answer means the flag stops working
precisely when the store has bad news — which is when it is wanted.

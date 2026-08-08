---
about: domains/coding/cli/src/verbs/sync.rs#differs
watch: [sig, logic]
---

# Which facet drifted matters, because the sealed reason should say which judgment moved

`differs` returns which specific facets changed (`probe`, `rules`,
`terminal`) rather than a single boolean "the declaration disagrees with
the anchor." "The probe was renamed" and "the transition table was
rewritten" are different judgments about why the criteria changed, and
whoever writes the `revise` rationale should be able to say which one it
was rather than a generic "declaration drifted."

## When this changes, ask

Does a new kind of criteria (beyond probe/rules/terminal) get its own
named facet, or does it get folded into an existing one it does not
actually match? A drift report that can't distinguish two different kinds
of change defeats the reason this returns a list of facets instead of a
bool.

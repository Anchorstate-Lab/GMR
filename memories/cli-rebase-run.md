---
about: console/cli/src/verbs/rebase.rs#run
watch: [logic]
---

# One rationale covers the whole `--all` batch, because one upgrade is one decision

`rebase` recaptures anchors against whatever instrument the build has now
(see [[verbs-recapture]] for what recapture actually does). A swapped
derivation makes the stored baseline incomparable to a fresh reading, and
treating that as comparable anyway would be changing criteria silently —
the substrate refuses to do that on its own, which is why `rebase` needs a
`why` at all. When `--all` recaptures several anchors in one call, they
all share the single `why` the caller gave, rather than each getting
prompted separately: one probe/tool upgrade is one decision, even though
it touches many anchors, and asking for N identical rationales would just
be noise.

## When this changes, ask

Does a new path let `--all` recapture different anchors under different
rationales in one invocation? If anchors are being rebased for genuinely
different reasons in the same run, that is more than one decision and
should be more than one `rebase` call.

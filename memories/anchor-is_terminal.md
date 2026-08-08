---
about: crates/gmr-core/src/anchor.rs#is_terminal
watch: [sig, logic]
---

# The substrate compares the status, it does not interpret it

Decision 4: there is no fixed status vocabulary, the status string is defined by
the domain, and the substrate only uses it for a terminal comparison.
`is_terminal` is that sentence in its entirety — take `state.status()`, look it up
in the `terminal` set. No prefix matching, no case folding, no understanding of
meaning.

The test using `"расчёт"` as a terminal status is not a joke, it is the criterion:
**the moment the substrate starts "reading" a status, it has quietly established a
vocabulary**, and that vocabulary will only ever cover the words whoever wrote it
happened to think of.

Decision 8 is mechanically honoured through this function: enter the terminal set
and you are `closed`, irreversibly (see [[journal-fold]]).

## When this changes, ask

Any string handling appearing here — trim, lowercase, `starts_with`, separators —
is interpretation. Ask: why can the domain not write the status the way it wants
it in the first place?

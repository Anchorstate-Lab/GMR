---
about: domains/coding/cli/src/verbs/sync.rs#ambiguous
watch: [sig, logic]
---

# Closing the old anchor is what answers the question, so closure has to be an input here

When a note drops one anchor key and gains another, `sync` refuses to guess
whether that was a rename or a typo, prints both halves, and tells the
reader to *close the old anchor with a reason, or put the old key back*.

That instruction used to be impossible to carry out. The refusal compared
the note's wants against the **binding record**, and closing an anchor does
not touch the binding record — it appends a terminal entry to that anchor's
own journal. So the reader followed the instruction, ran `sync` again, and
got the identical refusal, every run, forever. The only escape was a manual
`gmr bind`, which is the thing the safety valve exists to make people think
before doing.

So `closed` is a parameter: a dropped key whose anchor has been closed is
not an open question any more, because a human already answered it in the
one place the instruction pointed at. A dropped key whose anchor is still
live keeps blocking, and it keeps blocking **on its own** — one closed drop
does not excuse a live one, which is why `dropped` is filtered rather than
the whole check being skipped when any drop is closed.

`align_bindings` only asks `rt.read` about keys that are actually being
dropped, which is normally none. Reading every anchor to build a closed-set
up front would fetch every bound memory through its provider — the cost
that [[runtime-current-version]] is careful about, paid on every sync for a
branch almost nobody takes.

## Why an anchor that cannot be read still blocks

`rt.read` erroring — no such anchor in the journal — is not treated as
resolved. The key is in a binding record, so something bound it once;
"the journal has never heard of it" is a stranger state than a rename, and
silently letting it through would turn a typo'd key that never existed into
an automatic rebind.

## When this changes, ask

Does the new code resolve the ambiguity from anything other than closure?
Deletion of the note, absence of the coordinate, and a probe that stopped
finding it are all *observations*, not decisions — none of them is a person
saying "yes, I meant to move this." Only [[anchor-is_terminal]]'s closure
carries that, which is why it is the only thing that unblocks this.

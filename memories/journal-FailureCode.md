---
about: crates/gmr-core/src/journal.rs#FailureCode
---

# Both halves of "our failure" have to be enumerated as fully as each other

`ReasonClass` is the grain the substrate acts on. `FailureCode` is the grain a
human diagnoses with. Both are kept because they serve different readers.

**The two halves have to be symmetric.** A log that records seven kinds of probe
failure and one kind of rule failure is describing the tool's development history,
not the anchor's history — which half was taken seriously is legible from the
length of the enum.

`reason()` is derived from the code rather than stored alongside it, so the two
cannot disagree.

## When this changes, ask

Which half does the new code belong to? If it only ever grows the "probe failed"
half, stop and look at whether the rule-failure half should be getting finer too.
The asymmetry is itself the signal.

## Why `code` is an `Option`

`Entry::Attempt.code` may be absent for **one** reason only: entries written to
disk before codes existed do not have it, and never will — the log is
append-only. Those entries have to keep folding, they cannot turn into something
unreadable.

So `#[serde(default)]` here is not convenience, it is **a direct consequence of
append-only**. `reason` gets no such treatment, because it has been there since
day one.

Every new field has to answer the same question: entries already lying on disk do
not have it — can they still be read back?

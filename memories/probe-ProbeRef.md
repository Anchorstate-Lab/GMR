---
about: crates/gmr-core/src/probe.rs#ProbeRef
---

# What the anchor wrote down is not what actually runs

`ProbeRef` is **the name the anchor wrote down**; `Derivation` is **the thing that
actually runs on this machine**. They are not the same, and the gap has to be
kept.

In a fresh clone the declaration travelled over intact and the artifact did not —
so `ProbeRef` exists and `Derivation` cannot be resolved. That is exactly what
doctor's `stranded` is there to report, and exactly why "ask Artifacts instead of
asking the observer" gives a 100% false-positive rate: it takes a declaration and
puts the question to a repository that only knows about shell artifacts.

## When this changes, ask

Any code that merges the two, or derives one from the other → it has assumed
"what was written down is what runs". In a fresh clone and on a different machine
that assumption is false.

## `ProbeName` is a name, not a hash

It has to **survive an engine upgrade unchanged**. The declaration writes a name;
what this machine resolves it to is derivation's business (see
[[probe-Derivation]]). A name shaped like a version is rejected on the spot; the
criterion is in [[probe-check_probe_name]].

## The `name` slot used to be called `artifact`

The `#[serde(alias = "artifact")]` on the field is that sentence in its entirety.
Back when this slot held a version it was called `artifact`; entries written then
still say `artifact`, and they have to read back saying it — the log is
append-only, and a rename cannot invalidate old entries.

**Checked on the write path, not on the read path.** `check_probe_name` rejects 64
hex chars on the spot, and what those old entries carry in the name slot is
exactly 64 hex chars. The two do not conflict: the validator hangs off `try_new`
only, while `string_newtype!` generates a `#[serde(transparent)]` Deserialize that
validates nothing. **New names cannot get in; old entries can still be read back**
— that is a division of labour between two paths on the same newtype, not a
missing check.

## Deleting that alias does not go red in this repository

There is not one entry in this repository's log that says `artifact` (the store
was rebuilt after the rename). So deleting it leaves `cargo test` green and
`gate.sh` green, and the cost lands on older logs elsewhere: `name` has no
`default`, so without that key the whole `Entry` fails to deserialize and the
anchor becomes unreadable — not an error saying a field is missing, an entire
history that will not open.

**The only thing that will speak up is this anchor.** Deleting the alias reports
`signature-changed` on `ProbeRef` (attributes count as signature, see
[[ast-signature]]), and what comes back is this note. This constraint once lived
in [[probe-Derivation]] — which is anchored on `Derivation`, and `Derivation` does
not move for this edit at all. **A memory on the wrong anchor is a memory on no
anchor.**

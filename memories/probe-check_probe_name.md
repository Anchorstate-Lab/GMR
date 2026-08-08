---
about: crates/gmr-core/src/probe.rs#check_probe_name
watch: [sig, logic]
---

# A probe name is not allowed to look like a version

`ProbeName` is a **name**; `ProbeVersion` is an **earned hash**. In an anchor's
three identities they are different things (see [[journal-Versions]]). The name has
to survive an engine upgrade unchanged; the version has to move with its inputs.

So beyond the character-set check there is one more rule here: **64 hex chars are
rejected outright**. Nobody names a probe with 64 hex characters — that shape can
only mean someone pasted a version into the name slot. Without the check, an
anchor would declare a probe name that can never be resolved, and the error would
only say "no such probe".

## When this changes, ask

Loosening the character set → think it through: this name goes into
`declaration_hash`, becomes a filename, and ends up on a shell command line.
Removing the hex rejection → ask: the person who pasted the wrong thing, which
error message do they now see it from?

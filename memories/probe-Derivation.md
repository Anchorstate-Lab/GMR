---
about:
  - crates/gmr-core/src/probe.rs#Derivation
  - crates/gmr-core/src/probe.rs#Observes
  - crates/gmr-core/src/probe.rs#a_probe_that_says_what_it_emits_covers_every_path_below_a_named_field
  - crates/gmr-core/src/probe.rs#a_probe_that_cannot_say_what_it_reports_covers_everything_rather_than_nothing
  - crates/gmr-core/src/probe.rs#an_entry_written_before_a_probe_could_say_reads_back_as_unknown
watch: [sig, logic]
---

# An earned hash, not the bytes of a binary

`version` hashes **every input that can change the output** — the source files,
the versions of whatever parses them, the output contract. Decision 5.

**Not the bytes of a binary.** Bytes move with the platform and the compiler, and
an identity that says "the version moved although the behaviour did not" is noise
that nothing can filter out — every time someone changes machine, every anchor in
the repository reports "the probe changed", and everyone learns to ignore the
signal.

## When this changes, ask

Anything unrelated to "can the output change" got added to the version's inputs
(build time, machine name, target triple) → it is manufacturing that noise.
Conversely, an input that can change the output is missing from the hash → the
probe changed and nobody knows, which is far worse than noise.

## What earns a version is the transport's business

`Derivation` is only responsible for **carrying** the version and its
provability. How that version is computed — which source files are hashed, which
dependency versions are pinned — is decided by the concrete transport;
`coding-extract`'s `build.rs` is one example. The substrate does not dictate the
algorithm, only that it must close over its inputs.

When it cannot, the transport now says **what** it failed to close over rather
than only that it failed — `Verifiability::Open { over }`, see
[[probe-Verifiability]]. That is still the transport's business and not the
substrate's: the base owns the closed vocabulary of surfaces, and only the
transport knows which of them apply to it.

Shipping a new probe version: derivation moves, declaration does not. Those two
have to be separable, which is why they are two fields in [[journal-Versions]].

## `observes` says what comes back, and only some probes can

A transport used to say how a reading was derived and how far it could be
trusted, and nothing about **what fields it puts in `obs`**. So a domain that
wanted to check a rule against its probe had to write that list down a second
time — `probes.toml` carried an `obs = { at, facts }` beside every recipe, free
to drift from the program it described, watched by nothing.

`Observes::Named { fields }` is the transport's own answer. `covers` matches by
path segment, so a probe reporting `value` has said something about
`value.price_cents` and nothing about `valuey`.

`Observes::Unknown` is not a gap to be filled in later. Three of the six
transports run somebody else's program — shell, script, and the closures a
domain registers in-process — and this build cannot read what those print.
`covers` answers **true** for everything there: the check is on rules, not a ban
on shell probes, and "we have no grounds to refuse this" is the only honest
thing an unknown can say. `tools/gate.py` holds the roster of which family is
which, because nothing in the source tells "cannot say" from "did not bother".

The four built-in extractors already carried the list, as `Vocabulary.at` /
`.facts` — `Vocabulary::observes` hands that same list to the transport instead
of a second copy being written into a recipe file.

**It is skipped when unknown, and that is load-bearing.** `Derivation` rides
inside every journal entry, the journal is hash-chained over the canonical form,
and an entry that grew a key on the way back out would rewrite the hash of
everything behind it. A test pins the round trip.

## When this changes, ask

Does a transport that could say start answering `Unknown`? Every anchor behind
it opens without the check, and a rule reading a field the probe never reports
produces an anchor that observes forever and never transitions — see
[[runtime-open]].

Does `covers` start matching by characters rather than by path segment? Then a
probe reporting `value` accidentally vouches for `valuation`, and the refusal
that was supposed to catch a typo waves it through.

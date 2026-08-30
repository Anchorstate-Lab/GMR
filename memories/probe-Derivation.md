---
about:
  - crates/gmr-core/src/probe.rs#Derivation
  - crates/gmr-core/src/probe.rs#Observes
  - crates/gmr-core/src/probe.rs#a_probe_that_says_what_it_emits_covers_every_path_below_a_named_field
  - crates/gmr-core/src/probe.rs#a_probe_that_cannot_say_what_it_reports_covers_everything_rather_than_nothing
  - crates/gmr-core/src/probe.rs#an_entry_written_before_a_probe_could_say_reads_back_as_unknown
  - crates/gmr-core/src/probe.rs#undeclared
  - crates/gmr-core/src/probe.rs#a_field_the_program_prints_and_the_declaration_never_mentions_is_named
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

`Observes::Unknown` is not a gap to be filled in later, but it turned out to be
narrower than first drawn. Three transports **know**: the whole reading comes
back through `select::pick`. Two **relay**: they run somebody else's program and
carry a declaration written by whoever installed it — shell's rides in the
artifact manifest and inside the version it is addressed by, the in-process
one's is handed over with the closure. One **cannot**: `script` runs an
interpreter over a path with nothing describing it, and `covers` answers **true**
for everything there, because the check is on rules and not a ban on scripts.

`tools/gate.py` holds that roster and reads the body of `Transport::resolve`,
because nothing in the source tells "cannot say" from "did not bother", and every
one of these has fixtures that construct an `Unknown`.

The four built-in extractors already carried the list, as `Vocabulary.at` /
`.facts` — `Vocabulary::observes` hands that same list to the transport instead
of a second copy being written into a recipe file.

## A declaration that travels raises the stakes on it being true

`undeclared` is the other direction: what the program **reported** that the
declaration never mentions. Nothing else can catch it. `covers` refuses a rule
against the declaration, so a declaration the program has outgrown turns away a
rule reading something the probe demonstrably reports — and until the transport
could say anything at all, that failure did not exist.

It is checked at `open`, against the first real observation, and it is a warning
rather than a refusal: the reading is fine for every rule that reads a declared
field, and the anchor is worth having. See [[runtime-open]].

A declared field vouches for everything below it, which is `covers` read in the
other direction: a probe that declares `value` has said something about
`value.price_cents`, and a walk that reported it would flood on every reading.

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

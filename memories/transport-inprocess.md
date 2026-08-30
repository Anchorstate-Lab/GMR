---
about:
  - batteries/transport/src/inproc.rs#Extract
  - batteries/transport/src/inproc.rs#Registered
  - batteries/transport/src/inproc.rs#InProcess
watch: [sig, logic]
---

# `InProcess` runs a closure it did not write, so the registrant says what it left open

`Extract` is `Reach -> facts | null` — the same contract a subprocess probe
answers on stdout, minus the process boundary. `Reach` carries the cwd, the
position and the params a subprocess would have read from its environment, plus
the `Budget` a subprocess gets for free by being killable: it is the one slot
through which anything a call needs reaches the work, so widening it later costs
a field rather than a signature every implementor has to follow. What comes back
is `Result<Value, ExtractError>`, and `ExtractError` separates a budget that ran
out from a refusal, because the two become different `FailureCode`s in the
journal and a cancelled scan recorded as an ordinary failure would be a lie.

`Registered` pairs one such function with the hash of everything that can change
what it returns, with the sentence that hash cannot say, and with what the
closure puts in `obs`. `InProcess` decides none of the four — not which probes
exist, not what each version closure covers, not what it fails to close over,
and not what it reports: it carries the map assembly handed it.

`observes` is the registrant's for the same reason `verifiability` is. This
transport links a closure it has never read, so `Observes::Unknown` is what it
answers on its own behalf and what its own tests register. A domain that knows —
the four built-in extractors do, in `Vocabulary.at` / `.facts` — hands that list
in rather than writing it down a second time somewhere else, which is what a
recipe file used to be. See [[probe-Derivation]] for what the field buys.

Note what is deliberately *not* in `Reach`: nothing that changes the answer.
Adding the budget did not move any extractor's version, and must not — a
deadline decides whether there is an answer, never which answer it is. See
[[survey-narrow]] for the same line drawn on the other side.

## Why `Verifiability` is carried and not computed

Every other transport knows the operation it performs, so it can derive the
answer from something it controls: `file` opens a declared path and reads a
selector out of it, which reaches nothing, so `Closed`. `http` fetches a URL, so
`Open{Network, Clock}`. `sql` asks whether its source is a local SQLite file —
and a credential resolved from the environment never counts as local. `shell`
asks whether its manifest passes any environment through. `script` runs an
interpreter found on the host, so `Open{Interpreter, HostEnv}`.

`InProcess` calls an `Arc<Extract>` that assembly handed it. Whether that closure
reads an environment variable, opens a socket, or takes the clock is not
knowable here, and [[probe-Verifiability]] fixes what an answer would be
claiming: that `Derivation::version` has closed over everything that can change
the output. A transport that answers for a closure it has never read is making
that claim about code it cannot see.

Two things nearby look like they would settle it and do not.

`resolve` and `invoke` read the same map entry, so `ProbeName` resolves to what
actually runs. That is **identity**, and `Verifiability` reports **closure**:
`http` reads one entry in both and is still `Open{Network, Clock}`.

`Registered::version` is a hash of the closure's own source. That covers the
**instrument**, not what the instrument reaches for while running: `script`
hashes what it runs and is still open over its interpreter and its host.

So the field is required, with no default to forget. It moves the claim to where
the knowledge is; it does not make the claim checkable. What is checkable is that
somebody had to write it, and a registration site is small enough to review.

## Why the coding domain answers `Closed`

Its closures read the working tree under `Reach::cwd`. The tree is the **subject**
of the observation and not the instrument — the same line [[transport-file]]
draws when it declines to hash the file it reports on, because hashing the
subject would make every ordinary change read as a swapped instrument. `build.rs`
hashes the instrument. Nothing else varies: no clock, no socket, and the only
`std::env` in the extract crate is under `#[cfg(test)]`.

## When this changes, ask

A domain registers a closure that reaches outside the tree — a service, an
environment variable, a clock — and still writes `Closed`. Nothing here catches
that. The registration sites are the review surface, and there are two of them:
`bind` and `registry_uncached`.

Does the coding domain's answer still hold? Re-ask the moment an extractor reads
something that is not the tree under `cwd`, and whenever `registry_uncached`
stops being the same closure as the one `bind` links.

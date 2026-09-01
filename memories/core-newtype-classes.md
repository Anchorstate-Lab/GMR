---
about:
  - crates/gmr-core/src/addr.rs#check_sha256_hex
  - crates/gmr-core/src/addr.rs#a_minted_address_cannot_be_forged_through_the_wire
  - crates/gmr-core/src/addr.rs#an_admitted_name_is_not_refused_on_the_way_back_out_of_the_store
  - console/cli/src/rules.rs#key
  - console/cli/src/memories.rs#addressed_to
  - crates/gmr-core/src/memory.rs#check_provider_id
watch: [sig, logic]
---

# Two classes of newtype, because "where is this invariant enforced" has two answers

A newtype that offers an infallible `new` beside a validating `try_new`, and
derives `Deserialize` through `#[serde(transparent)]`, has a check nothing is
obliged to run: the cheap constructor is the one call sites reach for, and every
value read back from the journal, the binding store, a manifest or an install
index goes through neither.

Making `Deserialize` validate everywhere is the obvious repair and it is wrong.
The macro has two arms instead, and the arm a type takes is the answer to *where*
its invariant is enforced.

## minted: `ContentHash` · `ProbeVersion` · `FactAddress`

Every value is computed here, by `content_hash_of` or one of its callers, and
is a sha256 in lowercase hex. Nothing legitimate produces anything else, so
strict is free:

- `Deserialize` goes through `try_new`. A tampered manifest or install index
  fails to decode and says which field and why, instead of minting a version
  that flows into `Derivation`, into the journal, and into a `&s[..12]` slice
  somewhere downstream.
- `new` is `pub(crate)`. The public mints are `ProbeVersion::of(ContentHash)`
  and `FactAddress::of(ContentHash)` — you cannot hold one of these without
  already holding a hash, so there is no code path that computes a wrong one.
  Every production caller already had a `ContentHash` in hand and was calling
  `.into_inner()` to throw the type away.
- `short()` lives here too. Sixty-four hex characters is the type's guarantee,
  so the twelve-character form belongs to the type rather than being respelled
  as `&s[..12]` at each print site, where it is a panic waiting for the day
  something invalid gets in.

## admitted: `AnchorKey` · `StatusId` · `Kind` · `ProbeName` · `ProviderId` · `ExternalId` · `Version` · `SaidId`

These arrive from a person or from a provider, and their checks are *admission*
rules — a length ceiling, a character set. `Deserialize` stays permissive, on
purpose:

> **An admission limit belongs at the door a value comes in through, never at
> the one it comes back out of.**

The journal is append-only. A value already written is a fact about the past,
and validating on read means that tightening a limit turns an existing store
into one nobody can open — and the entries *behind* the offending one go with
it, because `entries` decodes the whole list. That is a worse failure than the
one being prevented, and it arrives long after the change that caused it.

So the limit is enforced where a value first becomes typed:

- `rules::key` — the one place `open` and `sync` mint an `AnchorKey` from text.
- `rules::terminal` — now fallible, because a terminal status seals an anchor
  irreversibly and the base matches it by equality.
- `memories::addressed_to`, behind `located` — an empty external id binds
  cleanly and then reports as `gone` forever, with nothing to restore.
- `Artifacts`' install index is typed `BTreeMap<ProbeName, ProbeVersion>`, so
  the *file's schema* is the door rather than a call site downstream of it.
  This one was reachable: `installed()` read strings off disk and minted a
  `ProbeVersion` from each.

`Notes` naming its own provider (`ProviderId::new(RESOLVED_THROUGH)`) is not a
door — a literal in this repository's own source is not admitted from anywhere.

## `ProviderId` carries a grammar, and that is what makes an address readable

`check_provider_id` is narrower than the other names here: lowercase, digits
and `-`, never leading with one, and never the word `said`. It is not tidiness.
`<prefix>:<rest>` has to be decidable as *an address* or *an id that happens to
contain a colon* from the text alone, and the only alternative is asking which
stores this run registered — which makes one string name two different records in
two runs (see [[cli-address-resolution]]).

`said` is refused for the same reason one step further out. `said:t7` names an
utterance ([[memory-Binding]]), and with `said` available as a provider name it
would *also* name a record in a store called `said` — the same ambiguity, one
level up from the colon, and resolved the same way: by the text, not by what this
run happens to have registered.

Being `admitted` is what makes tightening it safe: `Deserialize` is
transparent and `new` does not check, so every `Ref` already in a journal
reads back exactly as it was written. Only `try_new` sees the new rule, and
`try_new` is only reached at the doors — the CLI's address parser, and the
`providers.toml` loader, which now refuses a recipe whose name no address
could carry.

## The two tests are the classification

`a_minted_address_cannot_be_forged_through_the_wire` refuses four shapes for
all three minted types. `an_admitted_name_is_not_refused_on_the_way_back_out_of_
the_store` asserts both halves at once: a 400-character key still reads back,
and the same value is still refused by `try_new`. Either assertion alone would
let the class quietly flip.

## When this changes, ask

Does a new newtype get `minted` because its check is strict-looking, when its
values actually come from a user or a provider? That is the version of this
mistake that only shows up in somebody else's repository, on the day they hit
the limit — and it presents as a corrupt store, not as a rejected input.

Does a `minted` type gain a constructor that takes a bare `String`? The point
is not that the check runs, it is that there is nothing to check: see
[[probe-address]] for what these addresses are supposed to mean.

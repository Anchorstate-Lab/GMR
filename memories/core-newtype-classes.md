---
about:
  - crates/gmr-core/src/addr.rs#check_sha256_hex
  - crates/gmr-core/src/addr.rs#a_minted_address_cannot_be_forged_through_the_wire
  - crates/gmr-core/src/addr.rs#an_admitted_name_is_not_refused_on_the_way_back_out_of_the_store
  - domains/coding/cli/src/rules.rs#key
  - domains/coding/cli/src/memories.rs#addressed_to
watch: [sig, logic]
---

# Two classes of newtype, because "where is this invariant enforced" has two answers

`string_newtype!` used to have one arm. It emitted an infallible `new` beside a
validating `try_new`, and `#[serde(transparent)]` derived a `Deserialize` that
went through neither. The result was measurable: `AnchorKey::new` had 71 call
sites and `AnchorKey::try_new` had none, and every value read back from the
journal, the binding store, a manifest or the install index skipped the check
entirely. The predicates existed; no real path went through them.

The obvious repair — make `Deserialize` validate, everywhere — is wrong, and
the reason is the whole design. It splits the types in two.

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

## admitted: `AnchorKey` · `StatusId` · `Kind` · `ProbeName` · `ProviderId` · `ExternalId` · `Version`

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
- `memories::addressed_to`, behind `located` — an empty external id used to bind
  cleanly and then report as `gone` forever, with nothing to restore.
- `Artifacts`' install index is typed `BTreeMap<ProbeName, ProbeVersion>`, so
  the *file's schema* is the door rather than a call site downstream of it.
  This one was reachable: `installed()` read strings off disk and minted a
  `ProbeVersion` from each.

`Notes` naming its own provider (`ProviderId::new(RESOLVED_THROUGH)`) is not a
door — a literal in this repository's own source is not admitted from anywhere.

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

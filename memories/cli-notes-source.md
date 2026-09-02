---
about:
  - console/cli/src/notes.rs#Notes
  - console/cli/src/notes.rs#Claim
  - console/cli/src/notes.rs#Stated
  - console/cli/src/notes.rs#declared
  - console/cli/src/notes.rs#name_of
  - console/cli/src/notes.rs#versions_of
  - console/cli/src/memories.rs#claims_of
  - console/cli/src/memories.rs#stated_or_empty
watch: [sig, logic]
---

# The directory walk is one `MemorySource` among others now, not the only way in

`memories/**.md` used to be the only shape a memory could arrive in: `scan`
walked the filesystem, opened each file and parsed its frontmatter in one
breath. Nothing was wrong with any single step, but together they meant
"where memories come from" and "what a memory says about itself" could not
vary independently — and a mem0 user has neither a directory to walk nor a
frontmatter block to parse.

`Notes` is now one source of `Record`s and `scan` consumes them, so adding
a store is adding a source rather than editing this parser.

## Why it sits in the domain and not in a battery

Everything it decides is a domain decision: that notes live in one named
directory, that only `.md` counts, and that the grid a note speaks to GMR
through is its YAML frontmatter. It hands that grid over as an opaque
`Claim::Says` and reads none of it — `about`, `watch` and `links` are
interpreted in `claims_of`, one layer up. That split is what lets the same source serve a
domain with a completely different vocabulary, and it is the same line
[[content-discovery]] draws for every source.

## One of the two ways in is a trait, and the other is not

`MemorySource::list` is a trait method and `async`, because a store may be
across a network and `gmr memories` dispatches over every store that can
list. `declared()` is inherent and synchronous: it has one implementation —
this one — and every caller holds `Notes` concretely.

It was briefly a `Declaring` trait in `gmr-content`. Nothing dispatched on
it, so what the base was holding was this domain's vocabulary; the trait
went, the types came here, and [[content-discovery]] records what would
bring it back.

Synchronous is not an accident of having only one implementation. Routing
the declaration path through an async trait would make `Subscriptions::load`
async, and with it five call chains, to await a filesystem walk that never
yields — and, the reason that outlives this file, a declaration a network
can withhold leaves no roster to judge at all.

## The name is a property of the address, and this file owns the address

What a record should be *called* is neither retrieval nor discovery. `name_of`
sat on a base trait for one commit, which put this domain's rendering
vocabulary one layer below the domain.

It takes a `Ref`, not a `Record`, because rendering has an address in
hand and nothing else. Requiring a record would have meant a full scan every
time a memory is printed, which is on the path of `read` — so the name would
have cost a directory walk per line.

It answers `None` for a reference from any other store. That is not a
degenerate case: a repository whose notes are files can still bind memories
in mem0, and those have no name here, so they print as their address. The
alternative — inventing a name from a uuid — is the mistake
[[atlas-prose]] documents, one layer up.

## Versions come from one batched `git hash-object`

`scan` runs on `doctor`, `check`, `sync` and every subscription load. A
`Record` carries a version, and asking git once per note would have put a
subprocess per note on all of those paths. `git hash-object -- a b c`
answers for every path in one call, so it stays one subprocess per scan.

**When git cannot version a path, the scan fails rather than versioning it
some other way.** There used to be a content-hash fallback here, defended as
a degraded mode for repositories without git. Two things were wrong with it.
`git hash-object` needs no repository — it hashes a file wherever it is run —
so the case it was written for was not the one it fired in; what it actually
covered was git missing from `PATH`, where the git provider cannot version
anything either and nothing can be bound at all.

Worse, it made one provider answer with two version schemes. `sync` stamps a
binding with the version this source computed, and `read` compares that
against the version `Git::fetch` computes. Fall back once and the stored
version is a sha256 that `git hash-object` will never produce again: every
note reports as rewritten, forever, with a bound version nothing can
retrieve. A version arrived at a second way is not a degraded answer, it is
a wrong one, so this refuses and says why.

## The record and what it says are produced in one pass

`declared` reads a file, hashes it, and reads its frontmatter while it still
has the read error in hand — so a file it cannot open becomes
`Malformed("cannot read this file")` at the point that fact is known.

This was briefly two calls, `records` then `claim_of`, and the second one
had only bytes to work from: an unreadable file and an empty one both
arrived as none. The diagnosis was recovered by re-opening the file whenever
the bytes were empty — a second syscall, a window between the two reads
where the answer could change, and emptiness pressed into service as a
failure flag. One call removes all three, and [[content-discovery]] carries
the same reasoning for the contract that briefly imposed the split.

## A key present with no value is not the same as a key that is absent

Frontmatter is read as YAML into a `serde_json::Value`, and the vocabulary
is deserialised out of that value. The two libraries disagree about one
thing that matters here: `serde_yaml_ng` accepts a null where a sequence is
expected and produces an empty one, and `serde_json` rejects it. Real notes
write `anchors:` with nothing after it to claim nothing on purpose, so the
strict reading turned a deliberate, documented form into a `malformed`
lint.

`stated_or_empty` restores the lenient reading explicitly, on the three
sequence fields, rather than leaving it to whichever parser happens to be
in the path. The old behaviour was correct by accident; this one is correct
on purpose.

That difference was caught by the criterion this refactor was held to —
`doctor` and `sync --dry-run` producing byte-identical output before and
after — and by nothing else. It is the reason that criterion is worth the
trouble of capturing a baseline first.

## The source names the provider once, and nothing re-derives it

`Notes` stamps each record with the `Ref` it resolves through, and
`claims_of` carries that `Ref` onto the `Note` rather than reducing it to a
path. The provider used to be a constant in the domain, re-attached
downstream — which is the same value in a repository with one source and
the wrong value in a repository with two.

Naming it here is not the same hardcoding moved: this file is what decides
that notes are files in a git working tree, so which provider resolves them
is its own fact to state. What was wrong before was a *second* place
claiming to know it. See [[cli-sync-align-bindings]] for what that cost.

## When this changes, ask

Does `Notes` start interpreting what it reads — routing a coordinate,
checking a shape name, knowing what `watch` means? Then the grid and the
vocabulary have been welded together again, and the next source will have
to reimplement the vocabulary to be usable.

Does a second parser appear between the frontmatter and the domain's
types? Each hop is a chance for two libraries to disagree about null, and
the last one cost a working, documented form.

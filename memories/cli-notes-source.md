---
about:
  - domains/coding/cli/src/notes.rs#Notes
  - domains/coding/cli/src/notes.rs#records
  - domains/coding/cli/src/notes.rs#claim_of
  - domains/coding/cli/src/notes.rs#versions_of
  - domains/coding/cli/src/memories.rs#claims_of
  - domains/coding/cli/src/memories.rs#stated_or_empty
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
`Claim::Says` and reads none of it — `about` and `watch` are interpreted in
`claims_of`, one layer up. That split is what lets the same source serve a
domain with a completely different vocabulary, and it is the same line
[[content-discovery]] draws for every source.

## The trait impl delegates to a synchronous method, and that is the point

`MemorySource::list` is `async` because a store may be across a network.
This one is a directory. Routing `scan` through the async trait would have
made `Subscriptions::load` async, and with it five call chains, to await a
filesystem walk that never yields. So `records` is inherent and
synchronous, `list` delegates to it, and the contract is still satisfied —
demonstrably, since the impl exists and compiles against the same trait
mem0 implements.

## Versions come from one batched `git hash-object`

`scan` runs on `doctor`, `check`, `sync` and every subscription load. A
`Record` carries a version, and asking git once per note would have put a
subprocess per note on all of those paths. `git hash-object -- a b c`
answers for every path in one call, so it stays one subprocess per scan.

When git cannot run at all, versions fall back to a content hash. **That
fallback is not there to be right** — in a repository without git nothing
can bind anyway, because the git provider cannot version anything either.
It is there so that linting notes still works somewhere git does not, which
is a supported degraded mode `doctor` already reports.

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

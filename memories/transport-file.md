---
about:
  - batteries/transport/src/file.rs#Ask
  - batteries/transport/src/file.rs#version
  - batteries/transport/src/file.rs#inside
  - batteries/transport/src/file.rs#Shaped
  - batteries/transport/src/select.rs#pick
watch: [sig, logic]
---

# A config value is a decision somebody made, and the extractors cannot see it

M2's second family. The gap it fills was measured rather than assumed: anchoring
`deploy.yaml#replicas` today routes to the catchall, which reports

```
path matched, name did not — this reading is about whichever of 1 others was closest
```

It never found `replicas` at all. What it captured was an opaque fingerprint of
whatever it settled on, so editing `timeout_ms` handed the note back and editing
`replicas` was indistinguishable from it. A memory saying *why there are three
replicas* could not be attached to the three.

`file` answers the same shape of question as [[transport-http]] — a point and a
selector — with the bytes coming from the tree instead of the network. JSON, TOML
and YAML, picked from the extension unless `shaped` says otherwise.

## The version is the declaration, and the file is the subject

This is the one place the parallel with [[transport-script]] inverts and it
matters. A script's version *is* its file, hashed, because the script is the
instrument. Here the file is the **thing being measured**. Hashing it would move
the probe version every time the watched value moved, so every such reading would
come back `Incomparable` — the anchor could never once say the value changed,
which is the only thing it exists to say.

Two guards, because that mistake is one line away: a test resolves the same `Ask`
against two roots holding different contents and asserts the version is identical,
and `Ask` structurally holds no root — the root lives on `Files`, so `version()`
has nothing to open. If a root ever lands on `Ask`, read this paragraph first.

What is in: the path, the selector, and the format. A different format over the
same bytes is a different instrument and gets a different version.

## `NotFound` is the filesystem answering, not us failing

- **file absent** → `Outcome::NotFound`. The filesystem answered, as definitely as
  a 404. Filed as an error instead, the anchor backs off and retries a settled fact.
- **field absent from a file that is there** → `NotFound` too.
- **unreadable** (permissions, IO) → `Unreachable`. We could not look.
- **unparseable** → `Unusable`. Ours to fix, and not the same as absence.

## A declaration may not read outside the tree

`inside` refuses absolute paths and any `..` that escapes the root. Declarations
are reviewed — that is the authorization model D-8 leans on for the shell escape
hatch — but what this probe reads goes **verbatim into an append-only log**. A
path that can walk out of the repository is a way to commit `~/.aws/credentials`
where nothing can delete it. `Recorded::Digests` exists for facts that must not be
stored in the clear; it is opt-in, and this refusal does not depend on anyone
remembering it.

## When this changes, ask

Does the file's content start entering the version? Then every anchor in this
family reports `Incomparable` the first time its value moves, and none of them
ever report `changed` again.

Does `inside` gain an escape for "just this one absolute path"? Ask what stops the
next declaration from using it, and what deletes the log entry afterwards.

"""Layer 3 — proof that the assertions above still have teeth.

A gate's third way of dying is the quiet one: it stays green while testing
nothing. This repository has already lost two days to that exact failure, when
an editing pass truncated the old script mid-heredoc, `sh` treated the
unterminated block as ending at end-of-file, and the run kept exiting 0. The
answer then was a sentinel and a step count, which catches a truncated file and
nothing else. It would not notice `addressed_all` starting to return an empty
vector, and neither would seventy-three greps.

So each mutation below is a known break in the product paired with the promises
that must fail because of it. If a mutation lands and every promise still holds,
the gate is red -- not because the product broke, but because the gate did.

That has a second use worth as much as the first: a promise nothing here can
break is a promise nothing is really checking. The list is therefore also the
honest inventory of what this suite actually defends, and it does not drift the
way a paragraph describing the same thing would.
"""

MUTATIONS = [
    {
        "id": "the-memories-vanish-from-the-report",
        "file": "domains/coding/cli/src/verbs/observe.rs",
        "find": "    refs.iter().map(crate::memories::addressed).collect()",
        "replace": "    let _ = refs;\n    Vec::new()",
        "breaks": ["the-memory-comes-back-when-its-signal-moves"],
        "why": "a run that names no memory has handed the reader nothing",
    },
    {
        "id": "the-memory-arrives-with-no-content",
        "file": "crates/gmr-runtime/src/read.rs",
        "find": "    match std::str::from_utf8(bytes) {\n        Ok(text) => s.serialize_some(text),\n        Err(_) => s.serialize_none(),\n    }",
        "replace": "    let _ = bytes;\n    s.serialize_none()",
        "breaks": ["the-memory-comes-back-when-its-signal-moves"],
        "why": "an address without the bytes behind it is a filename, not a memory",
    },
    {
        "id": "every-memory-is-handed-back-every-time",
        "file": "domains/coding/cli/src/delivery.rs",
        "find": "            gmr::expr::Evaluated::Value(Value::Bool(on)) => Ok(on),",
        "replace": "            gmr::expr::Evaluated::Value(Value::Bool(on)) => Ok(on || true),",
        "breaks": ["a-memory-that-asked-about-another-axis-stays-put"],
        "why": "a reader buried in memories learns to discount all of them",
    },
    {
        "id": "a-refusal-is-swallowed",
        "file": "domains/coding/cli/src/verbs/check.rs",
        "find": "        unseen: !unseen.is_empty(),",
        "replace": "        unseen: false,",
        "breaks": ["a-spent-budget-refuses-and-never-becomes-state"],
        "why": "a signal nobody managed to look at must never read as a signal that did not move",
    },
    {
        "id": "a-rewritten-memory-reads-as-current",
        "file": "crates/gmr-runtime/src/memory.rs",
        "find": "        Grounding::Rewritten {\n            version: fetched.version,\n            content: fetched.bytes,\n            before,\n        }",
        "replace": "        let _ = before;\n        Grounding::Current {\n            version: fetched.version,\n            content: fetched.bytes,\n        }",
        "breaks": ["a-rewritten-memory-does-not-read-as-current"],
        "why": "a memory edited under its binding is the quietest way this system can lie",
    },
    {
        "id": "a-memory-loses-who-vouched-for-it",
        "file": "crates/gmr-runtime/src/read.rs",
        "find": "    pub sources: std::collections::BTreeSet<Source>,",
        "replace": "    #[serde(skip_serializing)]\n    pub sources: std::collections::BTreeSet<Source>,",
        "breaks": ["a-memory-can-be-traced-back-to-its-signal"],
        "why": "an argument whose backing nobody can read is not traceable",
    },
    {
        "id": "any-hit-counts-as-still-being-there",
        "file": "batteries/survey/src/matching.rs",
        "find": "    let identifies = |v: &[bool]| v.iter().zip(&gate).any(|(hit, id)| *hit && *id);",
        "replace": "    let identifies = |v: &[bool]| {\n        let _ = &gate;\n        v.iter().any(|hit| *hit)\n    };",
        "breaks": [
            "a-signal-that-is-gone-can-say-it-is-gone",
            "an-anchor-never-silently-takes-up-a-different-object",
        ],
        "why": "one hit on a category would again keep a dead coordinate alive, and an "
        "anchor would again take up whatever else shares its file",
    },
    {
        "id": "what-is-owed-is-only-what-is-owed-right-now",
        "file": "domains/coding/cli/src/verbs/mod.rs",
        "find": "        let raised = entries.iter().filter(|(seq, _)| *seq >= sealed).any(|(_, e)| {",
        "replace": "        let raised = entries.iter().filter(|(seq, _)| *seq >= sealed).last().iter().any(|(_, e)| {",
        "breaks": ["an-obligation-the-rules-put-away-is-still-not-discarded"],
        "why": "reading only the present would let a domain's own rules erase the record "
        "that a judgement was ever owed",
    },
    {
        "id": "migration-drops-what-it-was-carrying",
        "file": "domains/coding/cli/src/verbs/import.rs",
        "find": "",
        "replace": "",
        "breaks": ["migration-carries-the-whole-promise"],
        "why": "export and import are the only channel by which a memory crosses instances",
        "skip": "no single-line break in the import path is honest; needs a shaped fault",
    },
]


def live():
    return [m for m in MUTATIONS if not m.get("skip")]


def apply(root, mutation):
    path = root / mutation["file"]
    original = path.read_text()
    if mutation["find"] not in original:
        raise RuntimeError(
            f"{mutation['id']}: its anchor is gone from {mutation['file']}. "
            "The code moved and this mutation stopped meaning anything — re-aim it "
            "at what the code does now, rather than deleting it."
        )
    path.write_text(original.replace(mutation["find"], mutation["replace"], 1))
    return original


def revert(root, mutation, original):
    (root / mutation["file"]).write_text(original)

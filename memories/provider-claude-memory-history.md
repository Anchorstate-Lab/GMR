---
about: batteries/provider/src/claude_code.rs#this_provider_offers_no_history_at_all
watch: [sig, logic]
---

# "No history" stopped being an answer this provider gives and became a trait it does not implement

`ClaudeMemory` implements `ContentProvider` and nothing else. It does not
implement `History`, so `history()` takes the default and returns `None`,
and a caller holding this provider cannot reach `fetch_at` at all. That is
the whole mechanism — there is no branch anywhere that special-cases this
provider, no flag, and no escape hatch.

## What this replaced, and why the replacement is not cosmetic

This file used to be anchored to a `fetch_at` on this type that always
returned `Ok(None)`, and it argued that returning `None` was reporting a
true fact rather than standing in for an unimplemented method. That
argument was sound for whoever *wrote* it and silent about whoever *reads*
it: `git cat-file` failing on a collected blob produced the same `Ok(None)`
and therefore the same `retrievable: Some(false)`, so one value carried two
different worlds and `render` could only pick a sentence that was wrong for
one of them.

The deeper reason it existed at all is worth keeping. Nobody ever decided
to grant this provider an exception. It met the three load-bearing
requirements, could not meet the fourth, and the author wrote a comment on
`fetch_at` saying why. The "zero comments" rule then mechanically promoted
that comment into an anchored memory — and an implementation note, once it
has the same textual form as a section of `GMR.md`, reads like a design
decision. Nothing could tell the two apart, because **`GMR.md` §6's four
requirements had no mechanical check enforcing any of them.**

So the fix was not to this file. §6 now grades the four requirements —
three load-bearing, one a capability — and the capability's enforcement is
this trait split, landed in the same commit as the prose. See
[[provider-claude-memory]] for what this battery is otherwise allowed to do.

## When this changes, ask

Did `ClaudeMemory` gain a `History` impl? Only a real retrievable older
copy justifies one; synthesising history by keeping a private shadow copy
would make this provider start holding another product's data, which is
the one guarantee [[provider-claude-memory]] exists to keep.

And if this test is deleted rather than changed: nothing else asserts that
the absence is deliberate, and the next person to add `fetch_at` back has
no signal that it was ever removed on purpose.

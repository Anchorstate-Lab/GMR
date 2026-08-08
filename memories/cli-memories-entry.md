---
about: domains/coding/cli/src/memories.rs#Entry
watch: [sig]
---

# A bare key binds to an anchor that must already exist elsewhere

`Entry::Existing(String)` is the bare-key form of an `anchors:` list entry
— just a key, no `probe`/`position`/`shape`. It only binds; it never
declares. Per [[memories-lint]]'s `bare-key` lint, that key has to name an
anchor some other note's full declaration (or a hand-run `gmr anchor`)
already brought into existence, because nothing in this parser can
conjure a probe/position/shape out of a bare string.

## When this changes, ask

Does the new code let `Entry::Existing` synthesize a declaration on its
own (a default probe, an inferred position)? That would turn a bare key
from "bind to something declared elsewhere" into a second, implicit way to
declare, defeating the reason the lint flags it.

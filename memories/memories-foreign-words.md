---
about:
  - domains/coding/cli/src/memories.rs#foreign_words
  - domains/coding/cli/src/memories.rs#FRONTMATTER_WORDS
  - domains/coding/cli/src/memories.rs#a_header_in_another_products_format_is_louder_than_no_header_at_all
  - domains/coding/cli/src/memories.rs#a_misspelt_word_is_foreign_like_any_other
watch: [sig, logic]
---

# A header this format cannot read is reported, never translated

Every top-level key in a note's frontmatter is checked against the four
words this format has. Anything else is `unrecognised`, weighted `Breaks`,
and the fault names the foreign words and lists the four.

The failure this exists to catch is the quietest one available here. A
header that parses as YAML but says nothing this format knows declares no
anchor, so nothing observes whether the note still holds — and from the
outside it looks exactly like a note that declared something. A note with
no frontmatter at all is *louder*: it reports `unclaimed`. Serde's default
is to drop keys it does not know, which makes the worse case the silent one
and leaves the author no reason to look again.

## Why the check is explicit and not `deny_unknown_fields`

A strict `Frontmatter` would route a foreign header into the existing
`malformed` arm: weight `Blocks`, and serde's wording. Both are wrong. A
header written for another tool is not malformed — it is well-formed and
about something else — and its consequence is precisely `unclaimed`'s: the
note declares nothing. `Blocks` is reserved for what `sync` must refuse per
key; this has no key to refuse.

Owning the check also lets the fault name *which* words were foreign, which
is the difference between an author fixing a typo and an author re-reading
a serde error.

## Why the word list sits against the struct

`FRONTMATTER_WORDS` duplicates `Frontmatter`'s field names, and Rust will
not hand them over at runtime. Adding a field without adding its word makes
notes using it report `unrecognised` — loud and wrong. That is the safe
direction, and it is why the two must be read as one unit: the failure this
guards against is a header being accepted in silence, so the guard's own
failure must never be silence.

## When this changes, ask

Is anything starting to *translate* a foreign vocabulary — reading another
product's header and inferring an `about:` from it? That trades this
report for a guess. It is also unbuildable for the store it would be
written for: [[provider-claude-memory]] guarantees GMR never writes into
Claude Code's directory, so a coordinate hand-written into a file that
product rewrites would vanish without a word. Aboutness reaches GMR through
a binding, never through another store's header — the same line
[[cli-memories-entry]] draws for a listing.

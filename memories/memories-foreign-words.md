---
about:
  - console/cli/src/memories.rs#foreign_words
  - console/cli/src/memories.rs#FRONTMATTER_WORDS
  - console/cli/src/memories.rs#a_header_in_another_products_format_is_louder_than_no_header_at_all
  - console/cli/src/memories.rs#a_misspelt_word_is_foreign_like_any_other
watch: [sig, logic]
---

# A header this format cannot read is reported, never translated

Every top-level key in a note's frontmatter is checked against
`FRONTMATTER_WORDS`. Anything else raises `unrecognised` at weight `Breaks`,
naming both the foreign words and the five this format has.

A header that parses but names nothing this format knows declares no anchor
while looking, from outside, like it declared one — quieter than no
frontmatter at all, which reports `unclaimed`. That is the failure this
catches.

`Breaks`, not `Blocks`: a header written for another tool is well-formed and
about something else, and its consequence is `unclaimed`'s. `Blocks` is for
what `sync` must refuse per key, and this has no key to refuse.

The check is explicit rather than `deny_unknown_fields` so the fault owns
its code, its weight, and can name which words were foreign.

`FRONTMATTER_WORDS` duplicates `Frontmatter`'s field names because Rust will
not hand them over at runtime. A field added without its word makes notes
using it report `unrecognised` — loud and wrong, which is the direction this
must fail in: what it guards against is a header accepted in silence.

## When this changes, ask

Is anything starting to translate a foreign vocabulary — reading another
product's header and inferring an `about:` from it? [[provider-claude-memory]]
guarantees GMR never writes into Claude Code's directory, so a coordinate
hand-written into a file that product rewrites vanishes without a word.
Aboutness reaches GMR through a binding, never through another store's
header — the line [[cli-memories-entry]] draws for a listing.

---
about:
  - console/cli/src/prose.rs#walk
  - console/cli/src/prose.rs#found_in
  - console/cli/src/prose.rs#linked
watch: [sig, logic]
---

# The brackets arrive in pieces, so the text has to be put back together before the name can be seen

`[[other-memory]]` is this repository's own notation, not CommonMark. The parser
therefore has no reason to keep it whole, and it does not: `[[a]]` comes back as
five separate text events — `[`, `[`, `a`, `]`, `]` — because `[` is a link
opener that never found its closer. Scanning each event on its own finds nothing,
which is not a crash but a silent nothing: every wikilink in the corpus simply
fails to become an edge, and the page still renders.

That is why `walk` merges consecutive text events before anything looks for a
name. The merge stops at any non-text event, which is what keeps it honest —
inline code sits between text runs, so `` `[[a` `` and `` `b]]` `` cannot be
joined across the boundary into a link that was never written.

Code blocks are excluded from linkification for the same reason a quotation is
not an instruction: `memories/README.md` shows the notation to explain it, and
turning that demonstration into a live link would be reading a mention as a use.

There is a test asserting the parser *does* still split the brackets. It looks
redundant and is not: it is the only thing that will speak up if a future
pulldown-cmark hands `[[a]]` back whole, at which point the merge is dead weight
that nobody would otherwise think to remove.

## When this changes, ask

Does a new caller scan raw markdown for `[[` without going through `walk`? It
will work on short strings and quietly miss the ones the parser chose to split,
which is the worst shape this bug has.

## A name is resolved, never synthesised into an address

`target_of` used to turn `[[runtime-read]]` into `memories/runtime-read.md` —
a path template. It agreed with the node ids by coincidence, because both
happened to be git paths, and nothing anywhere said the two had to agree.

The coincidence broke the moment a record was addressed by anything else: with
memories in a store that names records by uuid, every link resolved to a path
no node answered to, and the whole reference graph — 143 edges in this
repository — silently became zero. Not one error, not one dead link on screen.
The page rendered perfectly and said nothing.

`linked` now takes a map from name to node id, built by asking the declaring
source what each record is called ([[cli-notes-source]]). A name nothing answers
to is written out as text with no destination, which is the honest rendering:
there is nothing to navigate to. The map is the only thing that decides, so
there is no second opinion to drift from the first.

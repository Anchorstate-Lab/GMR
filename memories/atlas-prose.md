---
about:
  - domains/coding/cli/src/prose.rs#walk
  - domains/coding/cli/src/prose.rs#found_in
  - domains/coding/cli/src/prose.rs#linked
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

## Where the name goes

`target_of` is where "a wikilink names a file under `memories/`" is decided. The
page marks a target it cannot resolve as dead rather than dropping it, so a
convention that stops holding shows up on screen instead of vanishing.

---
about:
  - "CLAUDE.md#GMR – Grounded Memory Runtime > 1. Immutable Core Rules (Owner‑set, do not re‑argue)"
  - "CLAUDE.md#GMR – Grounded Memory Runtime > 7. Owner‑Required Decisions"
---

# The criteria themselves

`CLAUDE.md` is the single source of the owner's positions and of what may not be
decided without them. These two anchors watch those two sections directly.

`fingerprint` watches both its axes by default — `missing` and `drift` — so a
section being edited and a section ceasing to exist both come back here.

The coordinates carry the document's own H1, because `prose-map` addresses a
section by its breadcrumb down the heading levels. A section here is
`CLAUDE.md#<H1> > <H2>`, not `CLAUDE.md#<H2>`; the shorter form names nothing and
reports `missing` forever.

## When the fingerprint changes, ask

There are only two possibilities: **the owner changed the criteria**, or **someone
changed what they should not have**. Observationally the two are identical and the
substrate cannot tell them apart — so it hands this section back to you, and you
say which it was.

The mechanism that rots is this third link: AI writes an argument → overturns the
owner's decision → the argument goes into the document → the next round reads the
document and takes the argument for a criterion. These two anchors watch that link.

This layer is **orthogonal to code granularity**. The anchors under `crates/` watch
whether some piece of code moved; these two watch whether the basis for judging
*whether that code should have moved* has moved. When the former changes, go look at
the code. When the latter changes, go re-read every memory.

## §7 is here by a judgement, not by an observation

§1 needs no argument: it is the thirteen owner-set rules, named as such.

§7 is watched because it is where "the ones nobody will notice being violated"
lives — deleting implementations or tests, changing crate boundaries, deciding what
direction an anchor watches, making a failure path unlogged, changing criteria.
Nothing about the file says that; somebody decided it. The grounds are sealed in
this anchor's `supersedes` rationale, which `gmr read` will hand back, because a
judgement about criteria belongs in a sealed record rather than in prose that can
be edited without leaving a trace.

## When this changes, ask

Does a new section of CLAUDE.md deserve its own anchor here, or is it covered by
one of these two? A rule nobody watches is prose, and prose only takes effect when
somebody reads it.

Does either of these get **closed** rather than **superseded**? Closing is how an
anchor at this layer disappears without anybody noticing: `check` skips a closed
anchor and `status` leaves it out of the listing, so the only thing that then
speaks is `unsupervised` on the record left standing on it (see
[[runtime-corpus]]). A section that was rewritten wants a new generation pointing
at what replaced it; a section that is genuinely gone wants somebody to say so in a
rationale. See [[anchor-Superseded]].

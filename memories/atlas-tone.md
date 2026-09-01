---
about:
  - console/cli/src/verbs/atlas.rs#anchor_tone
  - console/cli/src/verbs/atlas.rs#memory_tone
  - console/cli/src/verbs/atlas.rs#anchor_node
watch: [sig, logic]
---

# Colour is decided from what the domain already acts on, never from a table of status names

The obvious way to colour this page would be `match status { "drifted" => red, … }`.
It is the wrong way, and rule 4 is why: status strings are domain-defined, so such
a table is a second copy of `shapes.rs`'s vocabulary living somewhere nothing
checks. Add a `Dim` and the new status falls silently into the default colour.

So no status string is read here. `Tone` comes from two places that cannot go
stale that way:

- facts the substrate owns for every domain — `closed` (rule 8), `faltering`
  (our failure, see [[journal-faltering]]), `sighting` (the world's answer);
- `Subscriptions::delivers`, the question the domain *already* answers about
  whether an anchor is handing a memory back to a person (see
  [[delivery-standing]]).

`delivers` is asked with `moved: false` on purpose. A page written now is asking
"is anything still unhandled", not "did anything move during this run" — and the
carried axis bits answer exactly that. This is why an anchor can sit at a status
that is not `settled` and still be calm: `memories.rs#superfluous` moved on
`place` while its note watches only `sig` and `logic`, so nothing is being handed
back, so nobody needs to look. That is not a leak, it is the subscription working.

The badge is suppressed when the tone is calm. 298 of 308 anchors are `settled`;
a mark that appears on the ordinary case is not a mark. The word itself is never
lost — it goes to the inspector as a fact, so the vocabulary stays readable while
only the exceptions shout. Note the test that a status behind any *other* tone is
echoed verbatim, in a script this build has no business understanding: whatever
the domain called it is what gets drawn.

`memory_tone` reads `Grounding` for the same reason: it is one value the
substrate owns, and matching it exhaustively means a variant added there
cannot fall silently into a default colour. It used to test a combination
of `unavailable.is_some()`, `content.is_none()` and `retrievable ==
Some(false)` — three fields that could disagree, and did (see
[[runtime-grounding]]), so the page could paint a memory alarming for a
reason that was not actually true of it.

`Warrant::Holding` deliberately carries no tone, and the reason has changed
under it. It was: this holds for every memory bound before the last observation,
which after any `observe` is nearly all of them, so a tone flagged half the
corpus and made the channel useless.

That was a symptom, not a reason. The comparison was against the journal head,
and the head advances on entries that are not the world moving — a single failed
observation marked every memory on the anchor. [[runtime-moved-at]] fixed the
cause: `Moved` now fires when the state actually changed and not otherwise.

**So the suppression is now a choice rather than a necessity, and it is left
standing rather than quietly reversed.** Whether a memory whose ground genuinely
moved deserves a colour is a question about what this page asks a person to do,
which is the domain's to answer — not something to change as a side effect of
fixing the count.

## When this changes, ask

Did a status name, or a test for one, appear in this file? Then this page has
started keeping its own copy of the domain's vocabulary, and the next `Dim`
added to `shapes.rs` will be drawn in whatever colour the fallback happens to be.

Is a new tone being handed out for something no other verb asks a person to do
anything about? Every level above calm is a claim that someone should look. Make
that claim only where some existing verb already makes it.

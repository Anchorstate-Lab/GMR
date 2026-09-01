---
about:
  - console/cli/src/delivery.rs#delivers
  - console/cli/src/delivery.rs#a_note_that_says_nothing_takes_its_shapes_default
  - console/cli/src/delivery.rs#an_anchor_with_no_shape_and_no_watch_refuses_to_guess
watch: [sig, logic]
---

# Delivery asks "is anything still unhandled", not "did it move this time"

`check` used to recognise only `Observed::Transitioned`. A second observation where
the state had not moved was `Still` — nothing reported, exit code 0. **The
accumulated bits were fed to `status` and not to delivery.** Decision 1's "once set,
stays set until the person re-confirms" was honoured in the display layer and not in
the delivery layer.

The consequence was measured: this repository's `doctrine::red-cards` was broken (the
section it watched did not exist), `doctor` printed `section-gonex1`, and `check`
exited 0 — **CI was green**. After a signature change, running check twice gave
"nothing moved" the second time while `status` still had `v.sig` raised.

There are two paths now, **divided by whether the anchor has a shape, not by guessing
from what the state looks like**:

| | criterion | who decides |
|---|---|---|
| has a shape | is any subscribed bit set | bits accumulate; `accept` clears them |
| hand-written rules (`of()` returns `None`) | fall back to the edge: hand it back only if it transitioned this time | — |

The second has to stay. A hand-written rule table has nobody declaring what counts as
settled, so level triggering on it would mean "never green". Falling back to the edge
is a **known downgrade**, not an oversight.

There was once a third — table shapes leaned on a hand-written `settled` allowlist.
That was a product of the dual track: two answers to one question (does this state
still need a human), and the allowlist one had no subscriptions. Once every built-in
shape was vectorized it disappeared: settled means **every bit down**, derived rather
than listed.

**It asks the declaration, not the data.** `delivers` takes an `Option<&Shape>` and no
longer infers the kind of shape from whether the `state` has a `v` — that is
structural typing, and under it a hand-written-rules anchor and a table shape are
indistinguishable.

## A subscription is keyed by a `Ref`, not by a bare id

`delivers` takes the note's full address — provider and external id —
because a subscription belongs to one record in one store. Keyed by the
bare id, a note in a second store silently inherits the narrowing of a note
it merely shares a name with.

That failure is worth spelling out because of how it looks: the memory
simply stops being handed back, which is indistinguishable from the axis
not having moved. Nothing prints, nothing turns red, and the anchor reads
as settled. It is the quietest way this repository can fail, which is why
the case has its own test rather than resting on the type change alone.

## When this changes, ask

Adding an axis → ask: **after a person has seen this bit lit, is there anything left
for them to do?** If what they do matches an existing axis → merge them (criterion in
[[shapes-Dim]]). If there is nothing at all for them to do → it should not be an axis,
because settled means every bit down, and a bit that never needs a human keeps the
anchor unsettled forever.

There used to be a `settled` allowlist here, enumerating which statuses counted as
settled. It and the bit vector were two answers to one question, and the allowlist one
had no subscriptions — add a status to the vocabulary and forget the allowlist and the
anchor is handed back forever; forget to remove one and the anchor goes silent
forever. **One of two answers always gets forgotten, so keep only the derived one.**

The third path gets deleted (say, "if we do not recognise it, call it settled") →
hand-written-rules anchors go silent forever. The reverse, "if we do not recognise it,
call it unsettled" → they exit 1 forever. Both are wrong, so this has to be three
branches. And "do not recognise" now has a second meaning — the criteria drifted; see
[[check-drift]].

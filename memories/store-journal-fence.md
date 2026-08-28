---
about: crates/gmr-store/src/journal.rs#Fence
watch: [sig, logic]
---

# `Fence` records which lease generation wrote a row; it no longer decides anything

`Held(u64)` is the epoch one lease actually issued; `Unleased` names a write
made outside any lease. Both go into the hash chain ([[store-journal-chain]]),
so the log can say afterwards *under whose claim* each entry landed. Nothing
reads it to admit or refuse a write any more — that is
[[store-journal-expected]]'s job now, and [[store-journal-guard]] records why
the token could not do it.

## Still an enum, and for the same reason as before

`0` is not a sentinel for "no token". The two situations are different facts
about how a write happened, and an in-band sentinel would let "I hold no
token" and "my token happens to be epoch 0" read as one thing — in the hash
chain that would make two genuinely different provenances hash alike.

The chain currently spells `Unleased` as `0` when it hashes
(`fence.epoch().unwrap_or(0)`), which means a hypothetical epoch 0 would
collide there. No `Queue` ever issues 0 — `SqliteQueue` increments before
handing a ticket out, so the first epoch is 1 — and [[store-queue-fence]]
holds implementations to a strictly climbing counter, which keeps it that
way.

## Why it did not simply get deleted

A token that admits nothing still answers a question no other column can:
two writers wrote this anchor, and one of them held the schedule's claim.
That is worth keeping when someone is reading a log to work out what
happened. What is gone is the pretence that keeping it was what made
concurrent writing safe.

## When this changes, ask

Does anything start *deciding* with this value again? If a write path wants
to refuse something, the question is which of the two orthogonal questions
it is asking — permission, or premise — and the answer must not be spelled
in this type a second time.

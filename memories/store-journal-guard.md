---
about:
  - crates/gmr-store/src/journal.rs#guard
  - crates/gmr-store/tests/conformance.rs#journal_refuses_an_entry_folded_against_a_head_that_moved
  - crates/gmr-runtime/tests/operations.rs#an_entry_folded_against_a_head_that_moved_is_refused
watch: [sig, logic]
---

# One shared check, so the two backends cannot drift into disagreement

`guard` is a free function both storage backends call, rather than each
implementing its own. Written separately, the two versions would sooner or
later refuse (or admit) writes differently, and a concurrency bug is exactly
the kind of thing that stays invisible until two writers actually collide.
That reason has not changed; what it checks has.

It asks one question: **does the head this entry was folded against still
hold?** `Expected::Head(at)` is refused when the anchor's last seq is
anything but `at`. See [[store-journal-expected]] for why the premise has to
travel with the write at all.

## The head is this anchor's own, never the journal's

`seq` is global — one counter across every anchor, so a binding can date
itself against the log as a whole. The head compared here is
`MAX(seq) WHERE anchor = ?`, and it has to be: against the global head,
every anchor would conflict with every other one, and an untouched anchor
would be unwritable the moment anything anywhere was appended.

## It used to check the token, and that is now somebody else's job

Two branches lived here before. Both are gone, and neither loss is a
relaxation:

A **stale fence** (`Held(epoch)` below the high-water mark) named a lease
holder still working after its lease expired. Under a head check that writer
is already handled: if anything landed while it was away, its premise is
broken and it is refused; if nothing did, it is computing from a state that
still holds and its entry is honest, carrying a `taken_at` that says when it
actually looked.

An **unfenced observation on a leased anchor** was meant to be the second
writer a lease exists to prevent. It was also the reason a deployment with
no queue had **no concurrency control at all** while looking like it had
some — the same `Fence::Unleased` value said both "there are no leases here"
and "therefore nothing is checked". Correctness now comes from the premise,
which every deployment carries, so the queue went back to being a
scheduling device — see [[store-queue-fence]].

## When this changes, ask

Does the new check still compare against **this anchor's** last seq? And is
there any write path that reaches `INSERT` without passing through here —
one bypass, even a hand-triggered one, and the guarantee degrades to "most
of the time", with the writers on either side unable to see each other.

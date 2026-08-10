---
about:
  - batteries/transport/src/inproc.rs#invoke
  - batteries/transport/src/inproc.rs#a_panic_is_recorded_not_propagated
  - batteries/transport/src/inproc.rs#work_that_outran_its_budget_is_told_nobody_is_waiting
  - batteries/survey/src/cache.rs#scan
watch: [sig, logic]
---

# Giving up on the race is not cancelling, so the abandonment is handed to the work

`invoke` runs the extract function on `spawn_blocking` and races it against the
budget's deadline. Losing that race used to be the end of it, and the note here
said so: a blocking thread cannot be stopped from outside, so on timeout it kept
running to completion, unobserved, and that was called the price of not paying
for a process boundary.

It was a worse price than it looked. The CLI printed its error and returned while
the thread went on burning a core — measured at seven seconds of CPU after the
user had already been told the probe failed, and reported from a real repository
as several minutes of a pegged core that had to be found with `ps aux` and
killed.

So the race still cannot stop the thread, and no longer has to: on timeout
`invoke` calls `budget.cancel()` before returning. `Budget` carries an
`Arc<AtomicBool>`, the copy inside `Reach` shares it, and any extractor that
calls `budget.checkpoint()` between units of work sees `Spent::Cancelled` and
unwinds itself. That is the standard shape for `spawn_blocking` — the runtime
offers no way to interrupt a blocking thread, so the thread has to agree to
look — and it is the same shape rust-analyzer uses for a cancelled query.

`work_that_outran_its_budget_is_told_nobody_is_waiting` is the test that the
signal actually arrives, not merely that it is set.

## The reader is in another crate, which is why this note went stale

For one release the signal was carried to a door nobody opened: `invoke` set the
flag, and not one extractor read it, so an abandoned scan ran to natural
completion exactly as before. The reader now exists — it is the `budget.
checkpoint()` in `scan`'s walk, once per file, in `batteries/survey`. That is one
loop every extractor goes through, so no probe has to remember to look; see
[[probe-budget]] for why it was put there rather than into each `probe()` body,
and what the one version bump per probe bought.

**This note said the opposite for four commits after that landed, and nothing
caught it.** The anchors above watch `inproc.rs#invoke`, which did not change
when the reader appeared in another crate — a note describing a mechanism that
spans two crates cannot be anchored to only one end of it. That is why
`cache.rs#scan` is now in this note's `about:`, alongside [[probe-budget]]'s own
binding to it: two notes make claims about that loop, and both have to be woken
when it moves.

`shutdown_background` in the CLI is not made redundant by any of this — see
[[cli-main-run]]. The checkpoint fires between files, so a single enormous file
being parsed is still uninterruptible for as long as that parse takes, and a
process that has already printed its error must not wait for it.

## The deadline is absolute, and that is the point

`Budget` holds an `Instant`, not a `Duration`. A pass leases a batch and observes
it one anchor at a time; if each anchor restarted the clock, a batch of sixty
four would quietly be sixty four times its own budget. Handing the same `Budget`
to every anchor in the batch makes the batch cost what it says it costs. This is
the deadline-propagation rule gRPC and tower settled on, for the same reason.

## The panic boundary is load-bearing and predates all of this

A panic inside the extract function is caught through the `JoinError` from
`spawn_blocking` rather than allowed to unwind into the caller: a panic reaching
the top would take the whole process down, and a crash is not a journal entry.
Recording it as `ProcessFailed` is what keeps one bad probe from erasing
everything else the run was going to write down.

## When this changes, ask

Does the new code path still turn a panic into a returned `ProbeError` rather
than letting it past `invoke`? Any refactor that removes the `spawn_blocking` +
`JoinError` boundary — including making the extractor async — reopens the "one
probe's bug kills the whole run" failure this exists to close, and the boundary
has to be rebuilt somewhere explicit rather than assumed. And does the timeout
path still cancel the budget before it returns? Without that line the race is
back to being a notification.

Then ask the other half, which lives in another crate: **does anything still
read the flag?** Deleting the checkpoint from `scan` breaks nothing, compiles
clean, and passes every test in this file — the setter and the reader are joined
by a runtime convention, not by a type. `every_probe_stops_when_nobody_is_
waiting_for_it_any_more` is the only thing standing between that edit and a
silent return to a pegged core.

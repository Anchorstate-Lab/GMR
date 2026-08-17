---
about:
  - crates/gmr-runtime/src/policy.rs#content_budget
  - crates/gmr-runtime/src/policy.rs#content_call
  - crates/gmr-runtime/src/memory.rs#ground
watch: [sig, logic]
---

# A total for the operation, a slice for each record, and no counting of failures

One `read` walks the bindings on one anchor; `edges` and `read_all` walk
every binding in the repository. With a per-call timeout and nothing else,
that second shape is unbounded — a hundred stores taking their full call
budget each is a hundred call budgets. So there are two numbers, and they
answer different questions:

```
content_call_ms    how long may one store call take
content_total_ms   how long may one operation spend on content, all told
```

The total is minted once at the operation boundary — `read`, `read_all`,
`changed_since` — and each record is grounded through
`total.narrowed(call)`, which `Budget` already defines as "this span, but
never past the parent's deadline". Two properties fall out of that and both
are load-bearing:

- **A record that gives up does not take the others with it.** Narrowing
  passes the parent's cancel flag downward, never upward, so a slice
  running out is that record's answer alone.
- **Nothing new starts once the total is gone.** `ground` checks
  `remaining()` before reaching for a provider at all, so the answer for a
  record nobody had time to ask about is `Unreachable` with
  `BudgetSpent` — not-knowing, never `Gone`.

## Why not a circuit breaker

The obvious alternative is to stop asking a provider after N consecutive
failures. It was rejected because it makes a record's answer depend on how
many other records happened to be walked before it: the same repository,
the same store, the same moment, and a different traversal order produces a
different report. A budget has no memory of what failed — every record gets
the same slice of whatever is left, and the only input is the clock.

## What this does not do

The budget is a contract the provider is asked to honour, not a leash the
runtime can yank. `ground` will not *start* a call it has no time for, and
a provider that checkpoints will return promptly; a provider that blocks
forever while ignoring its budget still blocks forever. Cutting an
in-flight call short would mean an async timeout, and therefore a runtime
that depends on a specific async runtime — which is the thing `gmr-runtime`
does not do (see [[layers]]). So "honour your budget" belongs in the
provider conformance suite, where it can be checked, rather than in a
wrapper here that would only appear to solve it.

`Budget` itself lives in `gmr-probe` because it is the shared vocabulary
for every outbound call rather than a probe-only idea; content is its
second user. CLAUDE.md §5 says to move it out when a third appears.

## When this changes, ask

Does a new caller mint its own total, or reuse one it was handed? Two
totals in one operation means the operation has no bound, which is the
thing this exists to give it.

Does anything start counting failures again? That is the breaker coming
back under another name, and with it the dependence on traversal order.

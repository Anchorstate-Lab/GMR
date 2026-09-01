---
about:
  - domains/coding/cli/src/verbs/mod.rs#resolve_one
  - domains/coding/cli/src/verbs/mod.rs#pick
---

# Read-only verbs expand a prefix; verbs that change state refuse it

The CLI used to hand the user's string straight to `AnchorKey::new` — no parsing, no
existence check. All three kinds of bad input exited 2, and one of them was lying:

```
gmr check crates/gmr-core/src/addr.rs#write_aray
  → "the lease is held by someone else"
```

One letter misspelled, and what it reports is a lease conflict. The cause is that
`observe()` takes the lease first, and with no such row in the queue it returns
`Leased`, so it can never reach "there is no such anchor". **The error message stated
our failure (cannot find it) as a state of the world (somebody else is writing)** —
precisely CLAUDE.md's `NotFound is the world's answer / ProbeError is our failure`,
backwards.

## Where the line for prefix expansion falls

`resolve` expands a prefix; `resolve_one` refuses more than one. The line is not
"convenience", it is **whether one rationale can cover several judgments**:

| | prefix | why |
|---|---|---|
| `status` `read` `check` `observe` `health` | expand | looking at five anchors is looking at five anchors; no judgment |
| `close` `accept` `restate` `re*` `rebase` `requeue` | refuse | each is an independent judgment, and one `--why` cannot cover them |

`path:line` sits on the expand side for the same reason: `resolve` maps a
position to the anchor whose symbol starts at or above that line in that
file (file-level anchor as fallback), and the matched key is printed in the
answer, so a wrong guess is visible rather than silently acted on. Only
start lines exist in the facts, so the rule is containment by start —
precise spans would mean rolling every probe's earned hash for one
resolver's benefit.

This is the same rule as `accept --all` only pairing with `--criteria`: one declaration
change is one decision, while every baseline drift is its own. See [[shapes-Dim]].

## When this changes, ask

A verb moves from `resolve_one` to `resolve` → ask: does it write to the log? If it
does, one invocation will seal several records under the same rationale, which forges
"I made a judgment about all five of these".

`nearest`'s ordering gets replaced (it is longest common prefix today) → as long as the
correct key no longer sorts first when one letter is mistyped, the hint is worthless.
The test `a_typo_is_told_what_it_nearly_said` pins exactly that.

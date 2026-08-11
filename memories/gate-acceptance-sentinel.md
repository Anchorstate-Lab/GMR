---
about:
  - tools/gate.py#check_acceptance_intact
  - acceptance.sh#step
watch: [sig, logic]
---

# Only the sentinel proves the acceptance script ran; the gate proves the sentinel is checked

`acceptance.sh` was once truncated mid-heredoc by an editing pass. `sh` treats
an unterminated `<<'EOF'` as delimited by end-of-file, so the script parsed,
ran five steps, exited 0, and tested almost nothing — on two platforms, for two
days. `sh -n` does not catch it, and neither does reading the file.

What catches it is the counter. `step()` increments `$steps`, the last line
prints `ACCEPTANCE COMPLETE steps=$steps`, and CI greps for the exact number. An
unterminated heredoc swallows every line after it — the remaining `step` calls
*and* the final echo — so the run either prints nothing or prints a number
smaller than the one being grepped for. Silently stopping early and being
truncated fail the same way, which is the property worth having.

## Why the heredoc arithmetic was removed rather than fixed

The first version of this check also counted `<<'EOF'` openings against lines
that were exactly `EOF`, on the theory that an imbalance means an unterminated
heredoc. Three things were wrong with it:

- **It could not see nesting.** `acceptance.sh` writes shell scripts into
  heredocs. A marker inside a heredoc body counts the same as one that opens or
  closes a real one, and no amount of line-level counting fixes that — it needs
  a parser.
- **It balanced by accident.** The file has twelve `<<'EOF'` occurrences and
  eleven lines that are exactly `EOF`. It passed only because the twelfth is
  inside a `#` comment and the checker stripped comment lines first. Reword that
  paragraph onto a non-comment line and the gate goes red for no reason.
- **It was redundant.** Every failure it could catch, the sentinel catches at
  run time, exactly rather than approximately.

A check that passes for the wrong reason is worse than one that is not there:
it answers the question, so nobody asks it again.

## What the gate is actually for here

The sentinel runs in the acceptance job. If that job stops checking it, nothing
notices — which is the shape of the original bug one level up. So `gate.py`,
which runs in a different job, asserts the *mechanism* rather than the outcome:

- the last non-blank line is the sentinel echo
- `step()` really increments `$steps`, so the number is not a constant
- the workflow greps for the sentinel at all
- the number it greps for equals the count of `step ` calls in the script

All four were checked red before being kept.

## When this changes, ask

Is the thing being asserted the mechanism or the outcome? `gate.py` cannot run
`acceptance.sh` — it is a release build and a 1600-file fixture — so it can only
guarantee that something else will. If a check here starts trying to reason
about what the script *does* rather than whether its result is examined, it is
back to approximating a parser, which is where the removed count came from.

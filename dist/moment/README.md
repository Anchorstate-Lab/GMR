# The moment — one report, delivered where you already are

Everything in gmr is pull-shaped: `check` answers when asked. This directory
is the push half, and it deliberately lives outside the substrate — the base
invents no alerting; it makes "what is due" cheap to ask, and these adapters
ask it at the moments people and agents are already paying attention.

One report feeds every adapter:

```
gmr check --json      what moved on a watched axis; the memories handed back
gmr standing --json   which recorded conclusions no longer stand
render.mjs            both, as one markdown document (bodies included)
```

Default posture everywhere: **report, never block.** A due memory is a fact
awaiting a person's judgment, not an error. Every adapter has a strict switch
for owners who want a gate; none default to it.

## The five faces and their moments

| face | the moment | adapter |
|---|---|---|
| agent, mid-edit | right after touching a file, before its answer closes | `claude-hooks.json` |
| human, reviewing | the pull request | `action.yml` at the repo root |
| human, pushing | pre-push | `pre-push` |
| service, answering | inside the answer path | no tool — see below |
| ops, standing | the scheduled pass | no tool — see below |

### Agent (Claude Code)

Merge `claude-hooks.json` into the repository's `.claude/settings.json`.
After every `Edit`/`Write`, the agent is handed the axis state and bound
notes of that file's anchors — the same information that, arriving before an
answer's closure forms, redirects it instead of footnoting it. Read-only
(`status`, ~50ms after single-key narrowing); never `check`, which observes
and writes the journal and must not run as a side effect of every edit.

### Pull request

```yaml
- uses: actions/checkout@v4
  with: { fetch-depth: 0 }
- uses: Anchorstate-Lab/GMR@main
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    # strict: "true"        # owner's call; default reports only
```

The journal does not travel with a repository, so the action rebuilds it:
baseline at the merge-base, observe at the head, comment what moved — and
when nothing is due, say so ("N anchors observed · quiet"), because a
product whose success is silence has to make the silence visible.

### Pre-push

```sh
cp dist/moment/pre-push .git/hooks/pre-push && chmod +x .git/hooks/pre-push
```

Prints the same report before a push leaves the machine. `GMR_MOMENT_STRICT=1`
turns it into a gate.

### Service (Shape B) — the moment is already in the path

A service embedding the SDK does not need an adapter: its moment is the
answer itself. `sample` returns the reading with the address a citing answer
must carry; `bind`/`said` records what the answer rested on; `ground` reports
whether that ground still stands before the answer is reused. Grounding is a
property of the data flowing through the handler — there is no protocol step
to forget. What a service does need is the ops loop below, or `max_staleness`
on its reads, so the anchors it grounds against are actually observed.

### Ops — somebody has to run the observation loop

A deployment that only reads the event cursor sees "nothing changed" forever
and looks perfectly healthy — transitions land in the journal only when
something observes. Two doors, at least one must be open:

- reads carry `max_staleness` (`ground`/`sample`/`read --fresher-than-secs`):
  anchors are observed when touched; untouched anchors never update
- a scheduled `gmr pass` observes the whole corpus on cadence:

```
# cron — every 15 minutes, observe what is due, then let consumers read `since`
*/15 * * * *  cd /path/to/repo && gmr pass >/dev/null 2>&1

# launchd / systemd: wrap the same two commands in a timer unit
```

Consumers then ask `gmr edges --since <cursor> --json` (or the SDK's
`since`) and deliver wherever their face lives — that half is theirs on
purpose: which transitions matter, and to whom, is a judgment the substrate
refuses to ship.

# GMR — Grounded Memory Runtime

> Attach a subjective note to an objective, recomputable observation — and hand
> the note back when the world moves **along a direction you declared**.

**Status: under reconstruction.** The architecture was recently redefined (the
anchoring layer went from a stateless diff to a state machine) and the code is
still catching up. **The interfaces are not stable — don't depend on them yet.**

---

## The problem

You write down a judgment — *"this function's signature must not change"*,
*"this contract has to hold inside the service boundary"*, *"this metric should
cross a line by quarter's end"*. It was true when you wrote it.

Is it still true six months later? Nobody knows, because **nothing connects the
judgment to the part of the world it depends on**. A note system faithfully
hands the note back to you — including when it has already gone wrong.

## What GMR does about it

```
guaranteed      a note will not silently drift out of sync with the world
                along a direction its author declared
not guaranteed  along any undeclared direction. The system stays silent
```

That qualifier is not a disclaimer — it is **the design's primary risk**.
Whether you declared the right direction decides whether the system is worth
anything. GMR turns "memory quietly expiring" from implicit and unrecorded into
content-addressed, auditable, and revision-tracked. It does not make the problem
go away.

### What is worth anchoring

> Anchor where **facts constrain the judgment but do not decide it**.

- Facts fully decide the judgment → don't anchor. Just run it; the note is pure redundancy
- Facts don't constrain the judgment at all → nothing to anchor to; it degrades into prose storage
- **Facts constrain but don't decide** → anchor here

An interface signature constrains the contract *"passwords are hashed inside the
service boundary"* but does not decide it — the signature can stay put while the
implementation violates it. That gap is exactly why the judgment has to exist,
and it is where this system's entire value lives.

---

## Architecture

```
memory layer    references to subjective notes. Content lives with an external
                provider; GMR does not copy it
   │
anchoring layer a state machine — all of GMR's substance
   │
fact layer      the outside world. GMR neither stores nor ingests it
```

### The anchoring layer is a state machine

```
δ(state, obs, time) → state'
```

- `state` holds two things: **where the probe looks** + whatever the domain accumulates
- `obs` is the state vector the probe emitted this round
- state changed = one transition = one edge

Transition conditions are written as a **rule table** (guard → new state); the
first match wins. That table *is* the state machine's transition function:
readable, diffable, revisable rule by rule.

### Three parties

| | provides | form |
|---|---|---|
| **substrate** | state slot · transition bookkeeping · journal · sealing · terminal enforcement | code |
| **probe** | observation → { position, state vector } | a content-addressed executable; version = hash |
| **anchor** | which directions it cares about · transition table · terminal set | data — readable, diffable |

**Representation belongs to the probe; attention belongs to the anchor.** A probe
emits every direction it can see; an anchor declares which ones it cares about.
One probe therefore serves many anchors with different concerns.

**There is no fixed status vocabulary.** One domain's drift is reversible and
self-healing; another domain's settlement is irreversible and terminal. They
share no state machine, and the substrate will not choose for you.

The substrate interprets exactly one thing: **is this status in the terminal
set?** Everything else is computed, recorded, and notified — never acted on.

---

## Using it

The substrate ships no binary. A **domain** is what assembles it: probes, anchor
declarations, notes, and a CLI that picks a transport, a content provider, and a
storage backend. This repo carries one such domain — `domains/coding`, which
anchors this repository's own architecture — and its binary is called `anchor`.

```sh
cargo build --release            # binary: target/release/anchor
sh gate.sh                       # fmt · clippy · tests · substrate boundary checks
```

Every command takes `--repo <path>` (default `.`) and `--json`. State lives in
`<repo>/.anchor/memory.db`.

### 1. Declare anchors

Anchors are data, in a TOML file (`anchors.toml` by default):

```toml
[[anchor]]
key   = "surface::gmr-core"
probe = "batteries/probe-ast/target/release/ast-map crates/gmr-core"
position = { kind = "function", vis = "pub" }
rules = [
  'obs.exact == false => { position: state.position, n: 0, matches: [], status: "coordinate-missed" }',
  'not exists(state.n) => { position: state.position, n: obs.candidates, matches: obs.matches, status: "captured" }',
  'obs.candidates > state.n => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "added" }',
  'changed("matches") => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "moved" }',
]
terminal = []
```

| field | meaning |
|---|---|
| `key` | the anchor's name. Yours to choose; the substrate does not parse it |
| `probe` | what to run. Hashed — the hash *is* the probe's version |
| `position` | where the probe looks. Becomes `state.position`; the probe never returns it |
| `rules` | the transition table, `guard => new state`, first match wins |
| `terminal` | statuses after which the substrate refuses all further transitions |

```sh
anchor sync                      # open every declared anchor that doesn't exist yet
anchor sync --dry-run
```

`sync` **only opens new anchors — it never edits criteria.** If a declaration no
longer matches the anchor's live criteria, it says so and stops. Changing a probe
or a rule table is a revision with a sealed reason (below), not a refactor.

You can also open one directly:

```sh
anchor open surface::gmr-core \
  --probe 'my-probe crates/gmr-core' \
  --rule 'changed("shape") => { shape: obs.shape, status: "drifted" }' \
  --terminal settled
```

### 2. Observe

```sh
anchor observe                   # observe every anchor
anchor observe surface::gmr-core
anchor pass                      # observe only what the queue says is due
```

Each anchor reports `settled` · `moved` · `still` · `unseen` · `closed`.
Exit code is `1` when something moved — usable straight from CI.

### 3. Read what moved

```sh
anchor read                      # every anchor's current state
anchor read --moved              # only the ones that have moved or failed
anchor edges --since <seq>       # transitions / terminals / stalls since a journal point
anchor edges --status drifted
anchor health                    # per-anchor liveness
anchor doctor                    # anchors that never got seen, or carry no notes
```

`--since` is how a consumer asks "what changed since I last looked" cheaply. The
substrate invents no severity, priority, or alerting on top of it.

### 4. Bind notes

A note is a Markdown file in this repo; the binding says *what it is about*:

```sh
anchor bind memories/gmr-core.md --anchors surface::gmr-core,modules::gmr-core
anchor bind memories/gmr-core.md --detach
```

GMR stores the reference and the version it was bound at — never the content.
The content stays with the provider (git, here) and is fetched back by version.

### 5. Revise — every one of these needs a sealed reason

When an anchor reports a transition, you either change the code or change the
criteria. Changing the criteria is a judgment call, and it is recorded as one:

```sh
anchor reprobe      <key> --probe '<new probe>'   --why '...'   # look somewhere else
anchor retransition <key> --rule '<guard> => ...' --why '...'   # what counts as a change
anchor reterminal   <key> --terminal a,b          --why '...'   # what is irreversible
anchor restate      <key> --state '{...}'         --why '...'   # move the state directly
anchor close        <key> --why '...'             # retire the anchor
```

The journal is append-only; the substrate guarantees the reason is
**tamper-proof**, not that it is **sound**. A rubber-stamp revision looks exactly
like a real judgment in the data.

---

## Writing a probe

A probe is any executable. Under the shell transport it runs via `sh -c` from the
repo root, with a 30s timeout and a 1 MiB output cap.

```
input     $GMR_POSITION — the anchor's position, as JSON
output    stdout: one JSON object   = the state vector ("I looked, here it is")
          stdout: null              = not found      ("I looked, nothing there")
          non-zero exit             = unreachable    ("I could not look")
```

The last distinction is mandatory: *"nothing is there"* and *"I couldn't look"*
must never collapse into each other. Output over the cap is **rejected, not
truncated** — a truncated roster is precisely what hides "one went missing".

A probe may be arbitrarily complex, because its hash pins it. **Push what can be
computed into the probe; leave only the decision to the rule table.**

`batteries/probe-coord` is the shared convention for probes that report *fuzzy
coordinates*: several optional coordinate items plus "which matched and which
didn't". Exact addresses (line numbers, full paths, ordinals) die on the first
edit; fuzzy coordinates let one observation answer several questions at once —
renamed, moved, contract changed, or genuinely gone. This is **advice to probe
authors, not a rule of the substrate**: the substrate only knows there is a
position slot.

## Writing rules

Rules are evaluated by a deliberately small language: pure, terminating, no IO,
no clock, no randomness. Time comes only from observation fields or from moments
the journal already recorded — otherwise replaying one journal twice would give
two answers and the whole recomputable chain would be void.

```
roots        obs · state · taken_at · entered_at
operators    field paths · == != < <= > >= · and or not · + - * /
builtins     exists(<path>) · changed("<path>")
literals     numbers · strings · true · false · null · arrays · objects
```

Guards yield a boolean; the right-hand side constructs **the complete new
state** — the substrate does not merge, it replaces. Two field names carry
meaning outside the domain:

- `status` — the only field the substrate reads. Terminal sets and `edges --status` match on it
- `position` — where the probe looks next. Carry it forward, or the anchor moves

If a guard or a constructor blows up, the substrate **does not transition and
emits an edge**. The domain does not get to decide "let's skip this one" — no
path may exist that produces neither an entry nor an edge.

---

## Design constraints

- **Versions must be earned, not declared.** A probe's version is its content hash — a hash cannot lie, a hand-written version string can
- **Append-only is enforced by storage, not by discipline.** The journal refuses updates and deletes
- **The evaluation path touches no IO, enforced at the crate boundary.** Not by splitting traits — by the dependency list, which is mechanically checkable
- **Irreversibility is enforced by the substrate.** An invariant that domains uphold voluntarily inside their own rules is not an invariant
- **The substrate does not know value types.** The moment it understands "number" it will want thresholds, trends, priorities. Banding is done by opening more anchors

## Explicitly not doing

| | why |
|---|---|
| storing the fact layer | it is an external knowledge base |
| recomputing old observations under new rules | probe chains leave no archivable intermediate. **No solution** |
| shipping a list of "what you should detect" | anchor points are never enumerable, and the next direction someone actually cares about will not be on the list |
| severity / priority / alert thresholds | a threshold is a judgment; it belongs to the read model |
| making judgments recomputable | impossible. Only accountability is guaranteed |

---

## Repository layout

```
crates/      the substrate — domain-agnostic, produces no binary
batteries/   reusable implementations belonging to no single domain:
             transports, content providers, storage backends, shared probes
domains/     one domain: its probes, anchor declarations, notes, and the CLI
             that assembles the above
```

**Assembly is the domain's decision.** Which transport, which content provider,
which backend — hard-code those three into the substrate's shell and that shell
stops being domain-agnostic, no matter how domain-free its vocabulary reads.

## Documentation

- [`GMR.md`](GMR.md) — architecture SSOT
- [`CLAUDE.md`](CLAUDE.md) — decisions, red cards, dead concepts
- [`flow.svg`](flow.svg) — one observation end to end, and which layer owns each step
- [`modules.svg`](modules.svg) — module map: responsibilities and dependency direction
- [`memories/`](memories/) — this repo's own notes, bound to its own anchors

Note: `GMR.md`, `CLAUDE.md`, the CLI's output, and the inline comments are in
Chinese.

## License

[MIT](LICENSE) © 2026 Zongming-He

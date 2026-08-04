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
| **probe** | observation → { position, state vector } | any implementation a transport can reach; version = the hash of its semantic closure |
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
storage backend. This repo carries one such domain — `domains/coding` — and its
binary is called `gmr`.

Every command takes `--repo <path>` (default `.`) and `--json`. Declarations live
in `<repo>/.anchor/`; notes stay outside it, where people and agents will see
them. State lives in `<repo>/.anchor/state/`.

### The path, in five steps

```sh
gmr init                    # create .anchor/, install probes, report what is readable
                            # write a note naming the coordinate it is about
gmr sync                    # open what the notes declare, and bind them
gmr observe                 # has the world moved?  exit 1 if it has
gmr pass --json             # what moved, and the notes bound to it
```

`init` opens **no anchors**. What is worth anchoring is a judgment, and the tool
does not have it.

### 1. Write a note

The note is the entry point. One line of frontmatter says what it is about:

```markdown
---
about: src/auth.ts#createSession
---

# Sessions are only minted inside the service boundary
```

Everything else is derived: the probe from the file extension, the position from
the `#` split, the transition table from the `roster` preset, and the anchor's key
from the coordinate itself — so nobody has to invent a permanent identity on their
first note.

`sync` opens what the notes declare and binds them. It writes only when the
relation actually changed. When a note drops a key and gains an unseen one it
stops and reports, because that is either a rename or a typo and moving a binding
is a judgment call.

### 2. Declare an anchor directly (the explicit form)

For anchors no single note owns, or coordinates the minimal form cannot express,
`.anchor/anchors.toml`:

```toml
[[anchor]]
key   = "surface::gmr-core"
probe = "ast-map"                            # a name; never a version
params = { root = "crates/gmr-core" }
position = { kind = "function", vis = "pub" }
shape = "roster"                             # or spell out `rules = [...]`
terminal = []
```

| field | meaning |
|---|---|
| `key` | the anchor's name. Yours to choose; the substrate does not parse it |
| `probe` | the probe's name. What it stands for is local; the name is what travels, and it does not move when the engine behind it does |
| `position` | where the probe looks. Becomes `state.position`; the probe never returns it |
| `shape` | a named transition preset, expanded into literal rules at sync time |
| `rules` | the transition table written out, `guard => new state`, first match wins |
| `terminal` | statuses after which the substrate refuses all further transitions |

`sync` **only opens new anchors — it never edits criteria.** If a declaration no
longer matches the anchor's live criteria, it says so and stops.

### 3. Read what moved

```sh
gmr read                    # every anchor's current state
gmr edges --since <seq>     # transitions / terminals / stalls since a journal point
gmr health                  # per-anchor liveness
gmr doctor                  # anchors never seen, or carrying no note
```

`--since` is how a consumer asks "what changed since I last looked" cheaply. The
substrate invents no severity, priority, or alerting on top of it.

### 4. Revise — every one of these needs a sealed reason

When an anchor reports a transition, you either change the code or change the
criteria. Changing the criteria is a judgment call, and it is recorded as one:

```sh
gmr reprobe      <key> --probe <name>             --why '...'   # look somewhere else
gmr retransition <key> --rule '<guard> => ...'    --why '...'   # what counts as a change
gmr reterminal   <key> --terminal a,b             --why '...'   # what is irreversible
gmr restate      <key> --state '{...}'            --why '...'   # move the state directly
gmr close        <key> --why '...'                              # retire the anchor
```

The journal is append-only; the substrate guarantees the reason is
**tamper-proof**, not that it is **sound**. A rubber-stamp revision looks exactly
like a real judgment in the data.

### Building it here

```sh
cargo build --release        # binary: target/release/gmr
gmr probes build             # build and install the probe recipes (developers only)
sh gate.sh                   # fmt · clippy · tests · substrate boundary checks
sh acceptance.sh             # the whole chain, from a bundle, in a fixture TS repo
```

Users never run `probes build`: probes are built at release time and ship
prebuilt, with their recipe versions pinned in `recipes.json`. A user machine has
the artifacts but not the sources, and so cannot earn those hashes itself.

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

`batteries/probes/coord` is the shared convention for probes that report *fuzzy
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

- **Versions must be earned, not declared.** A probe's version is the hash of its semantic closure — every input that can change the output, and nothing else. A hash cannot lie, a hand-written version string can. Hashing a binary's bytes is the other error: platform and compiler move it while the behaviour stands still, and that version can never be compared across two machines
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
batteries/   reusable implementations belonging to no single domain: one
             package per role (transport, provider, probe artifacts), a
             new backend is a feature and a module, not a new package
domains/     one domain: its probes, anchor declarations, notes, and the CLI
             that assembles the above
```

**Assembly is the domain's decision.** Which transport, which content provider,
which backend — hard-code those three into the substrate's shell and that shell
stops being domain-agnostic, no matter how domain-free its vocabulary reads.

## Documentation

- [`docs/GMR.md`](docs/GMR.md) — architecture SSOT
- [`CLAUDE.md`](CLAUDE.md) — decisions, red cards, dead concepts
- [`docs/flow.svg`](docs/flow.svg) — one observation end to end, and which layer owns each step
- [`docs/modules.svg`](docs/modules.svg) — module map: responsibilities and dependency direction
- [`memories/`](memories/) — this repo's own notes, bound to its own anchors

Note: `docs/GMR.md`, `CLAUDE.md` and the declaration files are in Chinese; the
code and its comments are in English.

## License

[MIT](LICENSE) © 2026 Zongming-He

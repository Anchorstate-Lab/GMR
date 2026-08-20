# GMR – Grounded Memory Runtime

*Architecture SSOT: `GMR.md`*

---

## 1. Immutable Core Rules (Owner‑set, do not re‑argue)

1. The **anchor layer** is a state machine:  
   `δ(state, obs, taken_at, entered_at) → state'`
2. Probe position lives in `state.position` – set by the domain, not by plugins.
3. The shape of `position` is domain‑defined. The base reads `state.position` without interpreting its content.
4. No fixed state vocabulary. `status` strings are domain‑defined; the base uses them only for terminal comparison.
5. Plugin versions must be *earned hashes* – hashes over all inputs that can change the output, not over binary bytes.
6. Plugins emit `obs` (the **representation**). Anchors choose which directions to care about and write rules (the **attention**).
7. Transition conditions take only `state`, `obs`, `taken_at`, `entered_at` – no derived quantities from the base.
8. Terminal states are declared on the anchor; the base mechanically enforces irreversibility.
9. Memories are bound to anchors. *Binding* says “about what”; *subscription* says “when to hand it over”.
10. Transition conditions are written as a rule table: guard → complete new state; first match wins.
11. `state` is an addressable JSON structure, not a black box. The base can read fields but does not interpret their meaning.
12. Physical layers: the base produces no binary; batteries supply reusable implementations; domains own plugins, anchors, assembly, and CLI.
13. The expression language must be able to construct objects – each transition must produce a complete `state`, not a patch.

---

## 2. Bootstrap Data Is Not System Ontology

This repository uses GMR to supervise itself, hence it contains “usage data”:

- `memories/` – human‑written records. Frontmatter declares which coordinate they are about; anchors originate from these – **the sole source** of anchors in this repo.
- `.anchor/probes.toml` – probe recipes used by these anchors (name → what that name means).
- `.codegraph/` – local index of CodeGraph.

**No `.anchor/anchors.toml`.**  
It is optional; `gmr init` does not create it. A repo whose anchors come entirely from notes has no such file. (It existed briefly because legacy notes used bare keys that only bound, not declared; with full frontmatter `- key/probe/position/shape`, declaration and memory live in the same file, making that TOML redundant – any anchor not claimed by a memory is reported as *barren* by `gmr doctor`.)

Machine‑readable recipes live in `.anchor/`; human‑written memories stay outside. This is not aesthetic: TOML at the repo root would be mistaken as project code by Agents, while memories hidden in dot directories would be invisible – both break the system.  
`.anchor/` tracks only `probes.toml` in git; logs and artefacts are ignored (see `.anchor/.gitignore`).

These files are **not** the same layer as the GMR crates in `crates/`. Do not treat them as built‑in capabilities, default rules, product manifests, or crate dependencies.

**`architecture.toml` is not part of this set.**  
It is not read by any GMR code – not by probes, anchors, logs, or distribution. It is merely a dependency‑exclusion list for `gate.sh` (a hand‑written linter config). It is not made an anchor because whether a package depends on `tokio` is deterministically answered by `cargo tree` – a class of facts fully decided by reality, explicitly not subject to anchoring.

---

## 3. No Comments in Code

**Zero comments** in code – not “as few as possible”, but zero. Explanations go into anchored memories, anchored to the code coordinates.

Rationale: comments and memories are two copies that diverge, and nothing detects the drift. Memories are monitored by anchors and alert on code changes; comments are not. Leaving one comment is leaving a drift path that will never be caught.

One exception, and it is not a “comment”:

- `///` in `cli.rs` for clap – those are `--help` text, user‑facing strings that happen to use comment syntax. Removing them removes help.

`//!` module headers were an exception once. They stopped being one: a header saying “what this file is” drifts into saying “why”, and nothing observes it when it does. Whatever a header wanted to say belongs in a memory anchored to the code it is about.

This rule lives in `tools/gate.py` under “no comments in the clean zones”, not in this prose – prose only applies when read, but no anchor fires when comments creep back.  
Clean zones grow monotonically; once cleaned, add a line to `CLEAN_ZONES` in `tools/gate.py`, which is the list itself – do not restate it here, or this paragraph becomes a second copy that drifts.

---

## 4. What Each Change Affects

- Modifying `memories/` or `.anchor/probes.toml` = changing criteria or records for this repo as a GMR user; usually requires owner judgement.
- Modifying `crates/` = changing the GMR tool itself; must respect crate boundaries.
- Modifying `architecture.toml` = changing `gate.sh`'s gate criteria, unrelated to GMR semantics.
- Modifying `cliff.toml` = changing changelog text/grouping for whoever runs `git-cliff` by hand; not read by `gate.py` or any release workflow, and unrelated to GMR semantics. See §10.

---

## 5. Crate Boundaries

- **`gmr-core`**: vocabulary + content addressing + Entry + fold. Must not know how to fetch facts, evaluate rules, or store.
- **`gmr-expr`**: pure expression evaluation. No IO, no clock, no dependency on `gmr-core`.  
  (The obs‑strict / state‑lenient semantics and `changed()` convention are anchor‑layer decisions, not generic evaluator features – they merely happen to have no compile‑time dependency on `gmr-core`; do not read “no dependency” as “ignorant of anchors”.)
- **`gmr-probe`**: probe invocation contract. No concrete transport implementation. `Budget` also lives here: it is the shared vocabulary for every outbound call, not a probe-only idea, and `gmr-content` is its second user. Two users do not justify a crate of its own; a third does — move it then rather than growing a second budget vocabulary alongside it.
- **`gmr-content`**: retrieval and discovery contracts. What every store must do sits in `ContentProvider` itself; what only some can do gets its own trait (`History`, `MemorySource`), so declining a capability means not implementing it rather than answering "I have none". No concrete provider implementation, and no opinion about which store to enumerate or how much of it.
- **`gmr-store`**: storage traits and feature‑gated backends. Sliced by mutability: Journal / BindingStore / Sealer / LinkStore / Queue.
- **`gmr-runtime`**: sole orchestration layer. May depend on core / expr / probe / content / store, but must not make domain decisions.
- **`gmr`**: only re‑exports.

---

## 6. Design Principles

- Current state must come solely from journal projection.
- State vocabulary belongs to the domain; the base only implements terminal semantics.
- `position` lives in `state.position`; the base does not interpret its structure.
- Transition rules produce a complete `state`, not a patch.
- `NotFound` is the world’s answer; `ProbeError` / `Unevaluable` are our failures.
- The system allows no silent failure paths.

---

## 7. Owner‑Required Decisions

- Deleting real implementations or tests.
- Changing crate boundaries.
- Deciding what direction an anchor should watch.
- Making a failure path “not logged”.
- Changing criteria: probe, rules, terminal, state revision semantics.

---

## 8. Rust Discipline (Summary)

- Prefer types and constructors to express invariants; do not rely on comments.
- Public surface changes must state what new facts callers must know.
- `core/expr` remain pure; do not introduce IO, databases, clocks, or randomness.
- `runtime` must not hard‑code specific transports, content providers, or storage backends.
- New bugs: first write a reproducible test, then fix.
- After changes, run relevant `cargo test`; for boundary changes, run `gate.sh`.

---

## 9. Rust Engineering Discipline (Detailed)

- Let types express invariants. Prefer newtypes, private fields, validated constructors, and `Result`‑returning errors; do not rely on comments to constrain callers.
- Avoid exposing public fields for convenience – public fields expose invariants that are hard to retract later.
- Borrow when possible. Do not `clone` to solve ownership; clone only when ownership genuinely needs to fork, data is tiny, or semantics require a snapshot.
- APIs should preferentially take borrowed values: `&T`, `&str`, `&[T]`. Take owned values only when the function needs to hold the data.
- Return owned values with justification: new results, crossing async boundaries, storing in structs, or avoiding dangling references.
- Use enums to represent states and branches; avoid strings, bool combinations, or `Option`‑within‑`Option` to encode business states.
- Errors must have type boundaries. Library layers return structured errors; only CLI/render layers turn errors into human text.
- Avoid pointless traits. Abstract only when there are multiple real adapters, or when testing/assembly requires swapping.
- Leverage zero‑cost abstractions: `Iterator`, `match`, newtypes, generics make interfaces clear; do not introduce runtime boxing just for “abstraction”.
- Do not spread `async` unnecessarily. Only real IO boundaries are `async`; pure computation, fold, parse, eval remain synchronous pure functions.
- Avoid global state, implicit clocks, randomness. Pass time as a parameter, especially `core/expr` must not read a clock by itself.
- Write tests against public interfaces. Do not test private implementation details; if you must, the module interface is likely wrong.
- For every public surface change, ask: what new facts must callers now know? If the answer is too many, the interface is shallow.
- `clone` is not the default solution. First check if borrowing, `Arc`, `Cow`, or adjusting data flow can allow a single ownership path.
- But do not idolise zero‑clone: log entries, state snapshots, event records are semantically snapshots – clone them when appropriate. A `clone` must correspond to a semantics: snapshot, shared ownership, or preserving across lifetimes. Do not use `clone` to bypass borrowing design.
- Functions in `core/expr` should be as pure as possible: all inputs in parameters, all outputs in return values.
- `runtime` may own trait objects because it is the assembly layer; `core/expr` should not introduce `dyn` for “flexibility”.

---

## 10. Version & Release Process

One workspace version. major.minor is human‑only — a deliberate stability promise, not a fact any parser gets to infer from commit messages. patch belongs to CI alone.

- **Ordinary change**: do not touch `Cargo.toml`’s `workspace.package.version` at all. `.github/workflows/release.yml`’s `bump-version` job bumps the patch digit on every push to `main`, commits, tags, and releases it — a merge to `main` *is* a release. This only fires when the push actually touches a path that reaches the shipped binary or npm package (`crates/`, `batteries/`, `domains/`, `dist/`, `Cargo.toml`, `Cargo.lock`) — a push that only moves CI config, docs, or `tools/` produces no new tag, because the binary it would tag is byte-identical to the one already released.
- **Deliberate major or minor line**: edit `Cargo.toml`’s version to `X.(Y+1).0` (or `(X+1).0.0`) by hand, in the PR that earns it. CI sees `(major, minor)` has moved past the latest tag and tags exactly that version instead of bumping patch.
- `tools/gate.py`’s `check_version_bump` enforces the shape of a manual edit — major.minor must move strictly forward of the latest tag, patch must be `0` — it does not compute an expected version from commit messages. (It used to, via `git-cliff --bumped-version`; a squash‑merged PR collapsed `!`‑marked breaking commits into one non‑`!` PR‑title commit, git‑cliff read only that flattened title, and the bump size was wrong without gate.sh noticing. The fix was to stop asking a commit‑message parser to decide version numbers, not to make it read squashed history correctly.)

Nothing here is triggered by hand for an ordinary release — pushing the merge is the whole process. `cliff.toml` still shapes changelog text for whoever runs `git-cliff` manually; it is no longer read by `gate.py`.
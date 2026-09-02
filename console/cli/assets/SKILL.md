---
name: gmr
description: Use in any repository containing a `.anchor/` directory. GMR anchors what a repository knows to recomputable facts about the code — it tells you when a note has drifted from what it describes, and it holds your own conclusions to the readings they were built from. Trigger whenever `.anchor/` exists at the repo root: before trusting an old note, comment, or doc that claims something about specific code is still true, and when you have concluded something about the code that later work will rely on.
---

# gmr — grounded memory runtime

## What this is

An anchor is a small state machine watching one coordinate in the codebase (a function, a file, a schema field — whatever a probe can read). Two different things bind to it, and they are not the same kind of thing:

```
memory     a long-lived constraint someone wrote and reviewed    fails by drifting
inference  what one analysis concluded for the task in hand      fails by losing its ground
```

A **memory** is not derivable from the code — that is why somebody had to write it. When the anchor moves, the memory is not false, it is *due*: `check` hands it back and a person re-reads it and seals a reason with `accept --why`.

An **inference** is a sentence you produced this turn. It fails differently: the reading it was built from moved, or it was never built from that reading at all. Nobody has to re-read it — the condition its author wrote down simply stopped holding. `said` records one, `standing` asks whether they still hold.

GMR stores no memory content and no inference content beyond what you hand it. It reports **structure**: this claim is bound to these anchors, those anchors read differently now than when it was bound, this one cited a reading the anchor never took, the invariant its author wrote no longer holds. It never says the claim is therefore false — that is entailment, and it is yours.

An anchor watching a named symbol keeps a **vector**, one bit per axis: `missing`, `sig`, `logic`, `file`, `line`. A bit is set against the baseline *you last confirmed*, not against the last observation, so it stays set until you `accept`. Several axes moving in one edit all land — no axis is swallowed by another. `status` names the highest set axis; `v` shows them all.

`sig` and `logic` are read from the parse tree, not from the source text. Rewrapping a parameter list, adding a trailing comma, re-indenting a body or writing a comment moves nothing, so run the formatter without thinking about it. The flip side is the part worth acting on: if `sig` or `logic` did move, something a compiler would also see has changed — it is never just a reflow, and it is not worth `accept`ing without reading the note.

## Detection

If `.anchor/` exists at the repo root, this repository is managed by `gmr`. Don't look for `.anchor/anchors.toml` specifically — a repo whose anchors all come from notes with full frontmatter never creates one, and that's normal, not broken; it is the declaration channel for anchors whose memories live outside the repository. Before trusting any note, comment, or memory that claims something about the code is still true, check whether it's grounded and whether it drifted:

```
gmr status --json
gmr check --json
```

Every `--json` field that names a record — `check`/`observe`/`pass` under `"memories"`, `status` under `"note"`, `doctor`'s `gone` / `no_provider` / `unreachable` / `never_asked` / `no_before` — spells it `provider:external_id`, the address rather than a display name. `git:memories/auth.md` is a path in this repository; `mem0:9f8e…` is a uuid in a mem0 scope. Hand that string back verbatim to `gmr bind`, `reaffirm`, `cobound` and `link` — they take an address in place of a path and need no `--provider`; passing one that disagrees with the prefix is refused rather than guessed. Do not strip the prefix, and do not assume a memory is a file. (`gmr read` is not one of these: it takes an *anchor key*, never a record address.) Human-readable output spells the same record by its name — `auth` — so a note reads the way its author filed it; the address is what you compute with.

## The memory loop

This is a loop you run yourself as part of normal work — not a human-only ritual you wait to be asked for:

1. `gmr init` — creates `.anchor/`, installs bundled probes, and (if not already present) writes this skill doc to `.claude/skills/gmr/SKILL.md` in the project, or `~/.claude/skills/gmr/SKILL.md` with `--global`. Idempotent, safe to rerun — but it never overwrites a file that's already there, including this one, so re-running `init` after a `gmr` upgrade won't refresh a stale copy of this doc; delete it first if you want the bundled version back.
1b. `gmr adopt` — a repository that predates gmr is not empty of knowledge, only of anchors. This nominates what already exists — comments that read like constraints, documents that name real files — as ready-to-run `gmr anchor` lines. It writes nothing: each printed line is one judgment, run it or delete it. Licenses, TODOs and tool directives are filtered as non-constraints; everything else is yours to decide, which is the same line `accept` draws.
2. `gmr anchor src/auth.ts#createSession` — routes the coordinate to a probe, a shape and a position, declares it in `.anchor/anchors.toml`, and opens it. **It puts no memory anywhere.** The anchor now says "there ought to be a memory about this", and `doctor` reports it as `barren` until one is bound. Then name the memory, whichever way you keep them:
   - `--record <provider>:<id>` — you already wrote it, in your own memory system. One command declares and binds; nothing is copied into the repository.
   - `-m "sessions expire after 30 minutes because ..."` — you keep no memories of your own, so GMR writes one into this repository as a note. In git a note is the memory and the declaration in one file, which is why this path writes no `anchors.toml` entry.
   - neither — write the memory wherever you keep it, then `gmr attest <provider>:<id> --anchors <key>` (see below). This is the agent's road: you wrote the record, so you are the one vouching for the link.

   These are three ways to say which memory, not three kinds of anchor. Whatever the store, what happens afterwards is identical.
3. `gmr check` — did anything move on an axis a memory asked about? Exits 1 if so, and hands you the memories to re-read. It also flags two other conditions that make what it reports untrustworthy: anchors whose declaration no longer matches their live criteria (`gmr accept --all --criteria --why "..."` to take the new criteria), and anchors standing on a reading a different probe instrument took (`gmr rebase --all --why "..."` to recapture). Resolve those before trusting a quiet `check`.
4. `gmr accept <key> --why "..."` — you looked, and what it shows is the new baseline. Clears the vector, seals the reason. If both a baseline drift and a criteria drift are pending at once, plain `--why` refuses and asks you to say which with `--baseline` or `--criteria` — they're different judgments and don't share one reason.
5. `gmr status` — everything being watched, its axes, its memories. Reads only.
5b. `gmr memories [--provider <name>]` — what each store here will show you, and which of it is already bound. Reads only. This is how you find a reference to bind when the store is not a directory of files: a mem0 uuid is not something you can guess. A listing is what a store *will show*, not a roster of what exists — a record missing from one is not a dead reference. A store nothing here can enumerate gets a line saying so rather than being left out: it is not empty and not broken, and a record in it has to be named by an address you already hold. `--json` carries those under `cannot_list`.
6. `gmr atlas` — writes the whole anchor–memory graph to `.anchor/output/atlas.html`, one self-contained file. Reads only. Reach for it when the question is about *shape* rather than about one anchor: which coordinates several notes are all watching, which notes span many coordinates, what a subsystem's notes actually cover. Hand a person the path — the page is for reading, and `status --json` is the cheaper answer for anything you can resolve yourself.

`gmr anchor` with no coordinate opens whatever the declarations and notes already ask for — that is what a fresh clone needs, since the journal does not travel with the repository.

A note may pick its own shape, say which axes should wake it, and declare typed edges to other records:

```
---
about: src/auth.ts#createSession
shape: contract
watch: [logic]
links:
  rests-on: [session-lifetime, "mem0:9f8e21"]
---
```

`links:` maps a kind you choose (open vocabulary — write what you mean) to targets: a bare name is a note in this repository (`session-lifetime` → `memories/session-lifetime.md`), a `provider:id` address reaches any registered store. `sync` reconciles declared edges the way it reconciles `about:` — a declaration that disappears revokes the edge it derived, and an identical edge you asserted yourself through `gmr link` or the SDK is never touched. `doctor` prints a census of edges by kind and source.

## The inference loop — your own conclusions

Anything you conclude about this codebase and hand on — a finding, an answer, a claim in a PR description — rests on something you read. Record what you read *while you are reading it*, and `standing` can tell you later whether the ground moved under it.

```
gmr read <key> --json                     the anchor's state, and `fact_address`: the
                                          address of the exact reading it hands you
gmr said "<what you concluded>" \         one conclusion, held to that reading
  --on <key> --saw <fact_address> \
  --depends 'all(anchors, not state.v.sig)'
gmr ground                                do the recorded conclusions still stand?
                                          exit 1 if any does not
```

**Build the answer from what `read` returned, and cite the address it came with.** If you read the code yourself and then name an anchor, you have made two separate readings of the world and called them one; `standing` reports that as `unseen` — a conclusion built *beside* an anchor rather than through it. `said` warns about it at write time and records the conclusion anyway: what was believed is worth keeping even when it was believed badly.

`--saw` is optional and omitting it is honest — the conclusion is then recorded with nothing said about what you were looking at, and `standing` counts it separately rather than assuming.

`--depends` is one expression, in your words, that is true **while the conclusion still stands**. It reads the anchors the claim names:

```
all(anchors, not state.v.sig and not state.v.logic)
any(anchors, state.now.value.12.price_cents != 420)
count(anchors, state.v.roll)
```

Inside the quantifier, `state` is the anchor being asked about, so anything that works over one anchor works over the set. Say the narrowest thing that is true: `Holding` compares the whole state, so any edit to a watched coordinate reports moved, while a `depends` naming only what you relied on lets a finding about a signature survive an edit to the body. An invariant nothing in the world can reach — `true`, `1 == 1`, `all(anchors, true)` — is reported as `vacuous` and counted with the broken ones, not with the quiet ones.

`gmr ground` (older spelling: `standing`) prints, per conclusion, what each anchor's ground did, whether the reading you cited was one the anchor actually took, and what your invariant says now. It exits 1 when a conclusion's ground no longer settles it (`depends` broken, vacuous or unevaluable; or nothing stated and the ground moved) or when one cites a reading no anchor took. Conclusions that cited nothing are counted and reported but do not fail the run.

`gmr ground <id> --retire` stops asking about one. What it said stays in the table — an append-only record of what was believed — and nothing reads it again.

The two loops do not merge. A memory that drifted goes to a person; a conclusion that lost its ground is simply no longer supported, and you re-conclude rather than re-read.

## What's worth anchoring — judgment, not a rule

`gmr` ships no fixed list of "anchor this, not that," and this document doesn't add one either. The heuristic worth carrying in your head:

- A fact that fully decides the judgment on its own → don't anchor it, just check it directly.
- A fact that doesn't constrain the judgment at all → anchoring it won't hold; it degenerates into prose storage with extra ceremony.
- A fact that constrains the judgment but doesn't decide it → that's what anchoring is for.

Apply this yourself, in context, the same way you'd decide whether a comment is worth writing. Nobody — including this document — should be deciding it mechanically on your behalf.

## Verbs that seal a reason (`--why`)

`accept`, `close`, and the hand-driving verbs behind them (`reprobe`, `retransition`, `reterminal`, `rebase`, `restate`) all require `--why`, and it's sealed permanently into the journal. These are judgment calls being revised, not routine writes — give the real reason. It gets read later, by you or someone else, trying to reconstruct why the criteria changed. A terminal anchor cannot be un-terminaled; correcting a bad judgment means opening a new anchor with `--supersedes`, not fighting the old one.

## The verbs behind the front door

`gmr --help` shows ten. The rest still work and are reachable through `gmr help <name>`: `sync`, `open`, `observe`, `pass`, `read`, `doctor`, `health`, `edges`, `requeue`, `bind`, `attest`, `reaffirm`, `cobound`, `link`, `probes`, `publish`, `export`, `import`, and the five revise verbs. Reach for them to drive one part by hand, not for ordinary work.

`gmr read` serves the stored reading whatever its age. `--fresher-than-secs <n>` looks again first if the last sighting is older than that. It also takes a position: `gmr read src/auth.ts:120` answers with the anchor whose symbol starts at or above that line in that file (the file's own anchor as fallback), and prints the matched key so a wrong guess is visible.

`gmr health` reports, per anchor, whether it is pointed anywhere useful: how many times it was read, how many of those hand-backs a person answered, and how many of those answers involved rewriting a memory. An anchor that has never fired, or that fires and never changes a note, is watching a direction its notes do not care about — a `watch:` worth narrowing. It reports rates and never a verdict; where the line sits is the owner's call.

## Reading `gmr doctor --json`

```json
{"anchors": N, "live": N, "absent": [...], "unseen": [...], "barren": [...],
 "unseen_unreachable": [...], "unseen_unusable": [...], "unseen_unevaluable": [...],
 "unseen_never_asked": [...], "grounds": {"holds": {...}, "moved": {...}, ...},
 "stranded": [...], "undeclared": [...], "gone": [...], "no_provider": [...],
 "unreachable": [...], "never_asked": [...], "bound": N, "no_before": [...],
 "unverified": [...], "unsupervised": [...], "skill_stale": [...],
 "content_versioning": bool, "chain_break": N | null, "cache_fault": "..." | null,
 "provider_warnings": [{"provider": "...", "message": "..."}],
 "declared_providers": [{"provider": "...", "can": [...], "caveat": "..." | null}],
 "notes": [{"note": "...", "key": "...", "code": "...", "detail": "...", "breaks": bool, "blocks": bool}]}
```

**The exit code is decided by who can fix it, not by how bad it sounds.** Red: `stranded`, `provider_warnings`, breaking `notes`, `undeclared`, `gone`, `no_provider`, `skill_stale`, `unsupervised`, `chain_break` — a rebuild, an unbind, an edit, a re-init makes each of them go away. Never red: `unreachable`, `never_asked`, `no_before`, `absent`, `barren`, `unseen` — somebody else's service, a spent budget, or an ordinary state. A build failed over something the owner cannot act on only teaches them to stop reading the colour, so this list is the rule and not a tally.

- `barren` — anchors nobody has bound a note to yet.
- `absent` — the probe ran and found nothing there. Normal when criteria were written before the code exists — don't read this as "it used to be there and now it's gone" without checking.
- `unseen` — outstanding failed attempts, split by whose problem it is: `unseen_unreachable` (the probe could not be reached), `unseen_unusable` (it answered and the answer cannot be used), `unseen_unevaluable` (the rules cannot be evaluated against what came back), `unseen_never_asked` (our own budget ran out first).
- `grounds` — every bound record bucketed by what the ground under it did since it was bound: `holds`, `moved`, `incomparable` (a different extractor took the reading it is dated against, so a diff would answer "the instrument changed shape", not "the world moved" — `gmr rebase`), `absent`, `never_established`, `undated`. The human output additionally splits `moved` into records whose note subscribes to the axis that moved — `check` hands those back — and ones that moved on axes their `watch:` does not name.
- `stranded` — no transport here can resolve the declared probe (`gmr probes build`).
- `unsupervised` — every anchor these records are bound to has finished or was never opened. The record still claims something about the code and nothing observes it any more. Supersede the anchor, point the note somewhere still watched, or unbind it.
- `gone` — the store answered authoritatively that this record no longer exists. The binding points at nothing; unbind it or bind the record that replaced it. A store that merely *would not answer* is `unreachable`, never this.
- `no_provider` — a binding names a store this binary has no provider for. Yours to fix: enable the feature, set the credentials, or rebind.
- `unreachable` — a store would not answer this run. Reported, never counted.
- `never_asked` / `bound` — how many of the `bound` records this run never got to, because the total content budget ran out first. When this is non-zero, everything above it is a **partial view**; raise `--content-total-ms` to see the rest.
- `no_before` — rewritten records that cannot show what they said at binding time, because their store keeps no history or did not keep that version. You are still told they moved.
- `chain_break` — the journal's entry-to-entry links do not cover the entry at this seq. The log is append-only by database trigger, so something got past that or the file was edited underneath: do not trust readings at or after this point.
- `skill_stale` — an installed copy of this doc is not the one in the binary (it differs, or it cannot be read at all; never installed is neither). `gmr init` only ever writes it when absent, so an upgrade leaves the old text in place and agents keep reading contracts this build no longer honours. Both copies are checked — the project's and `~/.claude/skills/gmr/SKILL.md` — and the line names the one command that rewrites that copy: plain `gmr init` never touches the global one.
- `provider_warnings` — a content provider this binary tried to register at startup but couldn't (for example `claude-code` when `$HOME` isn't set). Bindings through it fail with "no provider named `<name>` is registered in this binary" until the underlying cause is fixed. That message means the store is unreachable from here, **not** that the record is gone — a provider that is registered and simply has no such record says "`<provider>` has no record `<path>`" instead. Check this before assuming a failed `gmr bind --provider ...` means the provider name was wrong.
- `declared_providers` — each store declared in `.anchor/providers.toml`, with what it can and cannot do here, said at assembly rather than left to be discovered by watching it fail.
- `notes` — lint findings over every file under `memories/`, independent of the anchors above. `breaks: true` means the note names no live anchor at all; `breaks: false` is advisory. Codes: `unclaimed` (no frontmatter, so nothing observes whether this note still holds), `bare-key` (an `anchors:` entry binds to a key without declaring it, and nothing else in the repo declares that key either), `long-hand` (an explicit `anchors:` entry states exactly what `about: <coord>` would already route to — safe to simplify), `retired` (the note names a shape/axis word this build no longer has — stale, or a deliberate record of something buried; only you can tell which), `unknown-shape` (a `shape:` this build does not have), `watch-invalid` (a `watch:` that does not parse, or that names a path no rule of that anchor ever writes), `watch-missing` (the anchor writes its own rules and neither it nor the note says when the memory should come back).

## Binding a memory you just wrote

When the memory is one *you* wrote — into your own memory store, a mem0 scope, anywhere that is not this repository's `memories/` — say what it is about in the same breath:

```
gmr attest <provider>:<id> --anchors src/auth.ts#createSession
```

Run it the moment the store hands the id back. It never asks the store to answer first, because a record too fresh to be readable is exactly when the link is most accurate and only you know it. The assertion lands as **self-attested** and every reader is shown that: you wrote the record and you are the only thing saying what it is about. That is worth recording — it is not a second opinion, and nothing here will pretend it is.

Run the same command again once the store can answer, and it stamps the baseline it could not take the first time. Use `attest`, not `reaffirm`: `reaffirm` records a person's judgement, and running it about your own memory would launder your say-so into somebody else's.

`attest` only ever adds. Ending a link is a judgement call, so it goes through `gmr bind <address> --detach`.

A note you write into this repository's `memories/` needs none of this — `gmr anchor` writes the file and its `about:` line, and the binding is *derived* from what the note declares about itself.

## Binding non-git content

`gmr bind <path> --anchors <key> --provider <name>` binds arbitrary content to an anchor, not only files inside this repo's own git tree. `--provider` defaults to `git`. Whether other providers (for example a `claude-code` provider reading this agent's own memory files) are available depends on how this particular binary was built — `gmr bind --help` reflects what's actually compiled in. `gmr memories --provider claude-code` lists that directory's `.md` files, ids being paths relative to it; GMR only ever reads there.

`reaffirm` and `cobound` take the same `--provider`. `link` takes `--from-provider` and `--to-provider` separately, because the two ends may sit in different stores — a memory in one store can contradict a memory in another, and saying so should not require moving either of them.

### A store nobody compiled in

`.anchor/providers.toml` teaches this binary a store without changing it:

```toml
[provider.desk]
fetch = "scripts/desk-fetch.sh"
list  = "scripts/desk-list.sh"   # omit it, and this store simply cannot be listed
ids   = "readable"               # or "opaque" — required; nobody but you knows which
```

`gmr doctor` says what each declared store can and cannot do at assembly, rather than leaving you to find out by watching it fail — and a store with `ids = "opaque"` and no `list` is told plainly that only memories written after it was wired up can ever be anchored, because nothing can enumerate it and its ids are not ones you could write down. `--json` carries this under `declared_providers`.

`fetch` is run with `GMR_POSITION` set to `{"id": "<external id>"}` and answers on stdout with `{"text": "..."}`, or `null` for a record that is not there — `null` means *gone*, and anything else the script can do about a failure is to exit non-zero, which reads as the store being unreachable. `list` answers `{"records": [{"id": "...", "text": "..."}, ...]}`. A memory is one JSON string: a store holding anything but text needs a provider compiled in, and so does one whose own version you need — a declared store's version is a hash of the text GMR computes itself.

### mem0

Set `MEM0_API_KEY` and the scope (`MEM0_USER_ID`, optionally `MEM0_AGENT_ID` / `MEM0_APP_ID`) before running any verb, and a `mem0` provider registers itself. With no key set, nothing registers and nothing complains — not using mem0 is not a misconfiguration.

Then `gmr bind <memory-uuid> --anchors <key> --provider mem0`. **The uuid is the whole reference**; mem0 keeps it stable when it updates a memory in place, so a binding survives the memory being rewritten — that rewrite is exactly what GMR is there to tell you about.

GMR only ever reads mem0. It does not write memories, metadata or anything else back, so a memory's `metadata` is never treated as saying which anchor it is about — declarations go through `gmr bind`. `gmr read` on a rewritten mem0 memory shows what it said at binding time, rebuilt from mem0's own change log.

`gmr memories --provider mem0` lists what that scope holds, marking what is already bound — reach for it rather than hunting a uuid by hand.

Two things worth knowing when reading a report about mem0 records: mem0's own consolidation can delete a memory that contradicts something newer, and that shows up as `gone` (worth acting on — the binding now points at nothing). Its store being unreachable shows up as `unreachable`, which is never counted against an exit code, because nobody holding this repository can fix it.

## Don't

- Don't hand-edit `.anchor/anchors.toml`, `.anchor/probes.toml`, `.anchor/providers.toml`, or anything under `.anchor/state/` — go through the verbs so the journal stays the one source of truth. `gmr anchor <coordinate>` is the verb that writes `anchors.toml`; it appends and never rewrites what is already declared, so a declaration you want changed is a criteria change and goes through `gmr revise` / `gmr accept --criteria`.
- Don't retry a failed transition condition expecting it to eventually succeed — an eval failure means the rule is wrong, not that the world is slow. Fix the rule.
- Don't compute a `--saw` address yourself, and don't fill one in from an anchor you did not read for this conclusion. An address you did not receive from `gmr read` is a second look at the world wearing the first one's name, and `standing` will report it as `unseen`.

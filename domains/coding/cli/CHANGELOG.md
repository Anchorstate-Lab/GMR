# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/Zongming-He/GMR/releases/tag/v0.3.0) - 2026-08-12

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(runtime,domain)* 兑现「无锚记录靠遍历被捎带」；删死旋钮；retain/cadence 给出到达路径
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- fmt
- reprobe/retransition/reterminal/restate become one revise verb
- read/observe stay separate from status/check — documented, not merged
- one classifier for "declared vs. live", four callers instead of four copies
- an unrouted note was reported as if it had been deleted
- deleting a note was how an anchor stopped being supervised, in silence
- one record for a note's failures, weighed once
- a note that says how to watch, and never says what, was the one failure nothing reported
- a declared probe was shadowed by a fallback that claims every extension
- scan and lint were two walks of the same directory; doctor only saw one kind of failure
- a bad watch: axis anywhere took Subscriptions::load down for everyone
- a coordinate about: could not route killed the whole scan, and doctor never saw it
- an unreadable cache is a fault to report, not a reason to stop
- cancellation runs down the tree; and two truths about the budget flag
- an anchor the budget never reached is skipped, not blamed
- raise the schema to v7 for a per-anchor probe budget, and climb into it
- a call carries a budget, and giving up on the race now cancels the work
- write the cache once per scan, not once per file
- release v0.2.3 ([#3](https://github.com/Zongming-He/GMR/pull/3))
- gmr check: stop re-walking and re-parsing the whole repo per anchor
- release v0.2.1 ([#2](https://github.com/Zongming-He/GMR/pull/2))
- bring SKILL.md and README up to date with the front-door verbs
- anchor its comments as memory, join the clean zone
- say when the baseline was taken by an instrument this build no longer has
- read out the diagnosis that has been in the journal all along
- a tombstone for every word this build stopped having
- say when the criteria drifted, and pin what a state may carry
- one declaration of what a coordinate probe emits, and no dead 98%
- one kind of shape, because a hand-written rule is not a kind of shape
- measure the surface a caller sees, and place as who you sit after
- resolve what the user typed into anchors that exist
- say what each axis answers, and let that decide when its bit falls
- a range every dimension has to hit, fired through the real probe
- six axes, each one a different thing to go and do
- report the breaking changes that used to leave no trace at all
- re-capture from a fresh reading, the way rebase already did
- check that a note is written the way the spec says, instead of trusting it
- ask what is still outstanding, not what moved this observation
- reserve `name` for what a coordinate can address on its own
- declare everything from notes, and stop capturing sections that missed
- point init, the skill and acceptance at the loop that now exists
- ask the observer what it can resolve, not a second copy of it
- six verbs at the front door, the other twenty behind it
- one router from a coordinate to a probe, a shape and a position
- hand a memory back only for the axes it asked about
- one verb for "I looked, and I accept what it shows"
- give an anchor a vector of accumulated bits, not one lossy status
- give string_newtype!'s try_new a structured error, not String
- make canonicalization honestly fallible instead of panicking
- Revert "gmr doctor: surface stale/rewritten/unavailable memories"
- gmr doctor: surface stale/rewritten/unavailable memories
- Surface provider-registration failures through gmr doctor instead of stderr-only
- Fix review findings on the agent-entry-point commit
- Add agent entry point: bundled SKILL.md and a Claude Code memory provider
- Fix Clippy: remove unneeded wildcard and unnecessary into_iter
- Commit current workspace changes
- rules get the same obs contract check a shape gets, whether written by hand or not
- a swapped instrument is a criteria change, so a person signs it
- a release is one file
- an artifact has two hashes, and only one of them is the rule
- a user's probe is a file in their own repository
- the extractors are the domain's, linked in, versioned by their closure
- the declaration slot holds a name; the transport answers what it stands for
- migrate this repo's declarations to probe names and shapes
- tarball is the primitive, npm is one wrapper
- name the fresh-clone failure instead of leaving N identical attempts
- rename the binary to gmr, and give every verb a line
- pinned recipe versions: probes resolve with no sources and no toolchain
- infrastructure only, and the journal moves to state/
- notes drive anchors: frontmatter is no longer decorative
- machine-read declarations move into .anchor/, notes stay outside
- recipe layer: anchors name a probe, not a machine-local hash
- ast-map 支持 TS/TSX/JS · Python · Go；shape 接进 sync
- shapes.rs：三个具名转换表预设与探针词表契约
- 记忆浮出：observe/pass 在锚动时交出绑定的笔记
- Update README and architect.md for the batteries/ restructuring
- provider-git -> batteries/provider with git as an opt-in feature
- transport-shell -> batteries/transport with shell as an opt-in feature
- Move retain/cadence out of the sealed Anchor into a settings store
- delete read's filter instead of replacing it
- record why an observation failed, on both sides of "our failure"
- replace read --moved with domain-given status filters
- rename has_lease to leases_configured, and cut this session's comment bloat
- stop telling users to revise what revise cannot touch
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Phase 3 (B2/B4/C1): stop re-folding the journal, one verb per file
- Phase 1: narrow Expr.source to String, dedupe env-var contracts
- Add Runtime::reaffirm() to close the write-side gap A2 left open
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- first commit

## [0.2.3](https://github.com/Zongming-He/GMR/compare/v0.2.2...v0.2.3) - 2026-08-08

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(runtime,domain)* 兑现「无锚记录靠遍历被捎带」；删死旋钮；retain/cadence 给出到达路径
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- gmr check: stop re-walking and re-parsing the whole repo per anchor
- release v0.2.1 ([#2](https://github.com/Zongming-He/GMR/pull/2))
- bring SKILL.md and README up to date with the front-door verbs
- anchor its comments as memory, join the clean zone
- say when the baseline was taken by an instrument this build no longer has
- read out the diagnosis that has been in the journal all along
- a tombstone for every word this build stopped having
- say when the criteria drifted, and pin what a state may carry
- one declaration of what a coordinate probe emits, and no dead 98%
- one kind of shape, because a hand-written rule is not a kind of shape
- measure the surface a caller sees, and place as who you sit after
- resolve what the user typed into anchors that exist
- say what each axis answers, and let that decide when its bit falls
- a range every dimension has to hit, fired through the real probe
- six axes, each one a different thing to go and do
- report the breaking changes that used to leave no trace at all
- re-capture from a fresh reading, the way rebase already did
- check that a note is written the way the spec says, instead of trusting it
- ask what is still outstanding, not what moved this observation
- reserve `name` for what a coordinate can address on its own
- declare everything from notes, and stop capturing sections that missed
- point init, the skill and acceptance at the loop that now exists
- ask the observer what it can resolve, not a second copy of it
- six verbs at the front door, the other twenty behind it
- one router from a coordinate to a probe, a shape and a position
- hand a memory back only for the axes it asked about
- one verb for "I looked, and I accept what it shows"
- give an anchor a vector of accumulated bits, not one lossy status
- give string_newtype!'s try_new a structured error, not String
- make canonicalization honestly fallible instead of panicking
- Revert "gmr doctor: surface stale/rewritten/unavailable memories"
- gmr doctor: surface stale/rewritten/unavailable memories
- Surface provider-registration failures through gmr doctor instead of stderr-only
- Fix review findings on the agent-entry-point commit
- Add agent entry point: bundled SKILL.md and a Claude Code memory provider
- Fix Clippy: remove unneeded wildcard and unnecessary into_iter
- Commit current workspace changes
- rules get the same obs contract check a shape gets, whether written by hand or not
- a swapped instrument is a criteria change, so a person signs it
- a release is one file
- an artifact has two hashes, and only one of them is the rule
- a user's probe is a file in their own repository
- the extractors are the domain's, linked in, versioned by their closure
- the declaration slot holds a name; the transport answers what it stands for
- migrate this repo's declarations to probe names and shapes
- tarball is the primitive, npm is one wrapper
- name the fresh-clone failure instead of leaving N identical attempts
- rename the binary to gmr, and give every verb a line
- pinned recipe versions: probes resolve with no sources and no toolchain
- infrastructure only, and the journal moves to state/
- notes drive anchors: frontmatter is no longer decorative
- machine-read declarations move into .anchor/, notes stay outside
- recipe layer: anchors name a probe, not a machine-local hash
- ast-map 支持 TS/TSX/JS · Python · Go；shape 接进 sync
- shapes.rs：三个具名转换表预设与探针词表契约
- 记忆浮出：observe/pass 在锚动时交出绑定的笔记
- Update README and architect.md for the batteries/ restructuring
- provider-git -> batteries/provider with git as an opt-in feature
- transport-shell -> batteries/transport with shell as an opt-in feature
- Move retain/cadence out of the sealed Anchor into a settings store
- delete read's filter instead of replacing it
- record why an observation failed, on both sides of "our failure"
- replace read --moved with domain-given status filters
- rename has_lease to leases_configured, and cut this session's comment bloat
- stop telling users to revise what revise cannot touch
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Phase 3 (B2/B4/C1): stop re-folding the journal, one verb per file
- Phase 1: narrow Expr.source to String, dedupe env-var contracts
- Add Runtime::reaffirm() to close the write-side gap A2 left open
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- first commit

## [0.2.2](https://github.com/Zongming-He/GMR/compare/v0.2.1...v0.2.2) - 2026-08-08

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(runtime,domain)* 兑现「无锚记录靠遍历被捎带」；删死旋钮；retain/cadence 给出到达路径
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- bring SKILL.md and README up to date with the front-door verbs
- anchor its comments as memory, join the clean zone
- say when the baseline was taken by an instrument this build no longer has
- read out the diagnosis that has been in the journal all along
- a tombstone for every word this build stopped having
- say when the criteria drifted, and pin what a state may carry
- one declaration of what a coordinate probe emits, and no dead 98%
- one kind of shape, because a hand-written rule is not a kind of shape
- measure the surface a caller sees, and place as who you sit after
- resolve what the user typed into anchors that exist
- say what each axis answers, and let that decide when its bit falls
- a range every dimension has to hit, fired through the real probe
- six axes, each one a different thing to go and do
- report the breaking changes that used to leave no trace at all
- re-capture from a fresh reading, the way rebase already did
- check that a note is written the way the spec says, instead of trusting it
- ask what is still outstanding, not what moved this observation
- reserve `name` for what a coordinate can address on its own
- declare everything from notes, and stop capturing sections that missed
- point init, the skill and acceptance at the loop that now exists
- ask the observer what it can resolve, not a second copy of it
- six verbs at the front door, the other twenty behind it
- one router from a coordinate to a probe, a shape and a position
- hand a memory back only for the axes it asked about
- one verb for "I looked, and I accept what it shows"
- give an anchor a vector of accumulated bits, not one lossy status
- give string_newtype!'s try_new a structured error, not String
- make canonicalization honestly fallible instead of panicking
- Revert "gmr doctor: surface stale/rewritten/unavailable memories"
- gmr doctor: surface stale/rewritten/unavailable memories
- Surface provider-registration failures through gmr doctor instead of stderr-only
- Fix review findings on the agent-entry-point commit
- Add agent entry point: bundled SKILL.md and a Claude Code memory provider
- Fix Clippy: remove unneeded wildcard and unnecessary into_iter
- Commit current workspace changes
- rules get the same obs contract check a shape gets, whether written by hand or not
- a swapped instrument is a criteria change, so a person signs it
- a release is one file
- an artifact has two hashes, and only one of them is the rule
- a user's probe is a file in their own repository
- the extractors are the domain's, linked in, versioned by their closure
- the declaration slot holds a name; the transport answers what it stands for
- migrate this repo's declarations to probe names and shapes
- tarball is the primitive, npm is one wrapper
- name the fresh-clone failure instead of leaving N identical attempts
- rename the binary to gmr, and give every verb a line
- pinned recipe versions: probes resolve with no sources and no toolchain
- infrastructure only, and the journal moves to state/
- notes drive anchors: frontmatter is no longer decorative
- machine-read declarations move into .anchor/, notes stay outside
- recipe layer: anchors name a probe, not a machine-local hash
- ast-map 支持 TS/TSX/JS · Python · Go；shape 接进 sync
- shapes.rs：三个具名转换表预设与探针词表契约
- 记忆浮出：observe/pass 在锚动时交出绑定的笔记
- Update README and architect.md for the batteries/ restructuring
- provider-git -> batteries/provider with git as an opt-in feature
- transport-shell -> batteries/transport with shell as an opt-in feature
- Move retain/cadence out of the sealed Anchor into a settings store
- delete read's filter instead of replacing it
- record why an observation failed, on both sides of "our failure"
- replace read --moved with domain-given status filters
- rename has_lease to leases_configured, and cut this session's comment bloat
- stop telling users to revise what revise cannot touch
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Phase 3 (B2/B4/C1): stop re-folding the journal, one verb per file
- Phase 1: narrow Expr.source to String, dedupe env-var contracts
- Add Runtime::reaffirm() to close the write-side gap A2 left open
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- first commit

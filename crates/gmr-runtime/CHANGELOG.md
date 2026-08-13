# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1](https://github.com/Anchorstate-Lab/GMR/compare/v0.3.0...v0.3.1) - 2026-08-13

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(expr,runtime,coord)* 三处「靠猜」的判断改成靠结构
- *(runtime,domain)* 兑现「无锚记录靠遍历被捎带」；删死旋钮；retain/cadence 给出到达路径
- *(store,runtime)* [**breaking**] 写入令牌覆盖全部观测路径；Fence 用类型表达「没有令牌」
- *(runtime)* 规则写错了第一次就响，不跟「世界够不着」共用退避
- *(expr,runtime)* changed() 不再吞掉拼写错误；开锚不再拒绝还没长出来的方向
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释
- *(core,runtime)* still 判定改吃折叠状态，ref_entry 指回被比对的那条记录

### Other

- release v0.3.0
- the budget cliff tests were flaking under a loaded workspace
- cache the per-file half, memoise the fold, stop paying for both
- an anchor the budget never reached is skipped, not blamed
- raise the schema to v7 for a per-anchor probe budget, and climb into it
- a call carries a budget, and giving up on the race now cancels the work
- release v0.2.3 ([#3](https://github.com/Anchorstate-Lab/GMR/pull/3))
- release v0.2.1 ([#2](https://github.com/Anchorstate-Lab/GMR/pull/2))
- anchor its comments as memory, join the clean zone
- read out the diagnosis that has been in the journal all along
- apply cargo fmt to pre-existing drift
- make canonicalization honestly fallible instead of panicking
- Surface provider-registration failures through gmr doctor instead of stderr-only
- debug
- Fix Clippy: remove unneeded wildcard and unnecessary into_iter
- a swapped instrument is a criteria change, so a person signs it
- verifiability says whether the closure is complete, not who checked the bytes
- the declaration slot holds a name; the transport answers what it stands for
- 记忆浮出：observe/pass 在锚动时交出绑定的笔记
- transport-shell -> batteries/transport with shell as an opt-in feature
- Move retain/cadence out of the sealed Anchor into a settings store
- record why an observation failed, on both sides of "our failure"
- extract gmr-content so a battery stops depending on the orchestrator
- delete watch_everything; an empty rule table stays empty
- rename has_lease to leases_configured, and cut this session's comment bloat
- always_full(anchor) becomes Anchor::retains_full()
- count revisions by a ChangeKind enum, not by a bare string
- Binding gains bound_at_seq: a snapshot of the anchor at binding time
- Phase 5 (D8): share the common part of revise/close's seal contexts
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Phase 3 (B2/B4/C1): stop re-folding the journal, one verb per file
- Phase 2 (B1): split Runtime into four capability-scoped services
- Phase 1: narrow Expr.source to String, dedupe env-var contracts
- Add Runtime::reaffirm() to close the write-side gap A2 left open
- Minimise Binding, demote bound_version to a store-layer view (RFC A2)
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- *(core,runtime)* 边沿从同一份折叠派生，不再手写第二份投影
- first commit

## [0.3.0](https://github.com/Zongming-He/GMR/releases/tag/v0.3.0) - 2026-08-12

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(expr,runtime,coord)* 三处「靠猜」的判断改成靠结构
- *(runtime,domain)* 兑现「无锚记录靠遍历被捎带」；删死旋钮；retain/cadence 给出到达路径
- *(store,runtime)* [**breaking**] 写入令牌覆盖全部观测路径；Fence 用类型表达「没有令牌」
- *(runtime)* 规则写错了第一次就响，不跟「世界够不着」共用退避
- *(expr,runtime)* changed() 不再吞掉拼写错误；开锚不再拒绝还没长出来的方向
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释
- *(core,runtime)* still 判定改吃折叠状态，ref_entry 指回被比对的那条记录

### Other

- the budget cliff tests were flaking under a loaded workspace
- cache the per-file half, memoise the fold, stop paying for both
- an anchor the budget never reached is skipped, not blamed
- raise the schema to v7 for a per-anchor probe budget, and climb into it
- a call carries a budget, and giving up on the race now cancels the work
- release v0.2.3 ([#3](https://github.com/Zongming-He/GMR/pull/3))
- release v0.2.1 ([#2](https://github.com/Zongming-He/GMR/pull/2))
- anchor its comments as memory, join the clean zone
- read out the diagnosis that has been in the journal all along
- apply cargo fmt to pre-existing drift
- make canonicalization honestly fallible instead of panicking
- Surface provider-registration failures through gmr doctor instead of stderr-only
- debug
- Fix Clippy: remove unneeded wildcard and unnecessary into_iter
- a swapped instrument is a criteria change, so a person signs it
- verifiability says whether the closure is complete, not who checked the bytes
- the declaration slot holds a name; the transport answers what it stands for
- 记忆浮出：observe/pass 在锚动时交出绑定的笔记
- transport-shell -> batteries/transport with shell as an opt-in feature
- Move retain/cadence out of the sealed Anchor into a settings store
- record why an observation failed, on both sides of "our failure"
- extract gmr-content so a battery stops depending on the orchestrator
- delete watch_everything; an empty rule table stays empty
- rename has_lease to leases_configured, and cut this session's comment bloat
- always_full(anchor) becomes Anchor::retains_full()
- count revisions by a ChangeKind enum, not by a bare string
- Binding gains bound_at_seq: a snapshot of the anchor at binding time
- Phase 5 (D8): share the common part of revise/close's seal contexts
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Phase 3 (B2/B4/C1): stop re-folding the journal, one verb per file
- Phase 2 (B1): split Runtime into four capability-scoped services
- Phase 1: narrow Expr.source to String, dedupe env-var contracts
- Add Runtime::reaffirm() to close the write-side gap A2 left open
- Minimise Binding, demote bound_version to a store-layer view (RFC A2)
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- *(core,runtime)* 边沿从同一份折叠派生，不再手写第二份投影
- first commit

## [0.2.3](https://github.com/Zongming-He/GMR/compare/v0.2.2...v0.2.3) - 2026-08-08

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(expr,runtime,coord)* 三处「靠猜」的判断改成靠结构
- *(runtime,domain)* 兑现「无锚记录靠遍历被捎带」；删死旋钮；retain/cadence 给出到达路径
- *(store,runtime)* [**breaking**] 写入令牌覆盖全部观测路径；Fence 用类型表达「没有令牌」
- *(runtime)* 规则写错了第一次就响，不跟「世界够不着」共用退避
- *(expr,runtime)* changed() 不再吞掉拼写错误；开锚不再拒绝还没长出来的方向
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释
- *(core,runtime)* still 判定改吃折叠状态，ref_entry 指回被比对的那条记录

### Other

- release v0.2.1 ([#2](https://github.com/Zongming-He/GMR/pull/2))
- anchor its comments as memory, join the clean zone
- read out the diagnosis that has been in the journal all along
- apply cargo fmt to pre-existing drift
- make canonicalization honestly fallible instead of panicking
- Surface provider-registration failures through gmr doctor instead of stderr-only
- debug
- Fix Clippy: remove unneeded wildcard and unnecessary into_iter
- a swapped instrument is a criteria change, so a person signs it
- verifiability says whether the closure is complete, not who checked the bytes
- the declaration slot holds a name; the transport answers what it stands for
- 记忆浮出：observe/pass 在锚动时交出绑定的笔记
- transport-shell -> batteries/transport with shell as an opt-in feature
- Move retain/cadence out of the sealed Anchor into a settings store
- record why an observation failed, on both sides of "our failure"
- extract gmr-content so a battery stops depending on the orchestrator
- delete watch_everything; an empty rule table stays empty
- rename has_lease to leases_configured, and cut this session's comment bloat
- always_full(anchor) becomes Anchor::retains_full()
- count revisions by a ChangeKind enum, not by a bare string
- Binding gains bound_at_seq: a snapshot of the anchor at binding time
- Phase 5 (D8): share the common part of revise/close's seal contexts
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Phase 3 (B2/B4/C1): stop re-folding the journal, one verb per file
- Phase 2 (B1): split Runtime into four capability-scoped services
- Phase 1: narrow Expr.source to String, dedupe env-var contracts
- Add Runtime::reaffirm() to close the write-side gap A2 left open
- Minimise Binding, demote bound_version to a store-layer view (RFC A2)
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- *(core,runtime)* 边沿从同一份折叠派生，不再手写第二份投影
- first commit

## [0.2.2](https://github.com/Zongming-He/GMR/compare/v0.2.1...v0.2.2) - 2026-08-08

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(expr,runtime,coord)* 三处「靠猜」的判断改成靠结构
- *(runtime,domain)* 兑现「无锚记录靠遍历被捎带」；删死旋钮；retain/cadence 给出到达路径
- *(store,runtime)* [**breaking**] 写入令牌覆盖全部观测路径；Fence 用类型表达「没有令牌」
- *(runtime)* 规则写错了第一次就响，不跟「世界够不着」共用退避
- *(expr,runtime)* changed() 不再吞掉拼写错误；开锚不再拒绝还没长出来的方向
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释
- *(core,runtime)* still 判定改吃折叠状态，ref_entry 指回被比对的那条记录

### Other

- anchor its comments as memory, join the clean zone
- read out the diagnosis that has been in the journal all along
- apply cargo fmt to pre-existing drift
- make canonicalization honestly fallible instead of panicking
- Surface provider-registration failures through gmr doctor instead of stderr-only
- debug
- Fix Clippy: remove unneeded wildcard and unnecessary into_iter
- a swapped instrument is a criteria change, so a person signs it
- verifiability says whether the closure is complete, not who checked the bytes
- the declaration slot holds a name; the transport answers what it stands for
- 记忆浮出：observe/pass 在锚动时交出绑定的笔记
- transport-shell -> batteries/transport with shell as an opt-in feature
- Move retain/cadence out of the sealed Anchor into a settings store
- record why an observation failed, on both sides of "our failure"
- extract gmr-content so a battery stops depending on the orchestrator
- delete watch_everything; an empty rule table stays empty
- rename has_lease to leases_configured, and cut this session's comment bloat
- always_full(anchor) becomes Anchor::retains_full()
- count revisions by a ChangeKind enum, not by a bare string
- Binding gains bound_at_seq: a snapshot of the anchor at binding time
- Phase 5 (D8): share the common part of revise/close's seal contexts
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Phase 3 (B2/B4/C1): stop re-folding the journal, one verb per file
- Phase 2 (B1): split Runtime into four capability-scoped services
- Phase 1: narrow Expr.source to String, dedupe env-var contracts
- Add Runtime::reaffirm() to close the write-side gap A2 left open
- Minimise Binding, demote bound_version to a store-layer view (RFC A2)
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- *(core,runtime)* 边沿从同一份折叠派生，不再手写第二份投影
- first commit

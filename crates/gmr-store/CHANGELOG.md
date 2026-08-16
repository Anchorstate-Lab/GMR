# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.3](https://github.com/Anchorstate-Lab/GMR/compare/v0.3.2...v0.3.3) - 2026-08-16

### Added

- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(store,runtime)* [**breaking**] 写入令牌覆盖全部观测路径；Fence 用类型表达「没有令牌」
- *(store)* 退场不再清零令牌计数器
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- release v0.3.2 ([#7](https://github.com/Anchorstate-Lab/GMR/pull/7))
- release v0.3.1 ([#5](https://github.com/Anchorstate-Lab/GMR/pull/5))
- release v0.3.0
- prove the shipped rung on the shipped schema, not only on a toy one
- decide the migration inside the write lock, not before it
- raise the schema to v7 for a per-anchor probe budget, and climb into it
- carry an older database across instead of refusing it
- apply busy_timeout via SqliteConnectOptions, not a migration query
- release v0.2.3 ([#3](https://github.com/Anchorstate-Lab/GMR/pull/3))
- gmr check: stop re-walking and re-parsing the whole repo per anchor
- release v0.2.1 ([#2](https://github.com/Anchorstate-Lab/GMR/pull/2))
- anchor its comments as memory, join the clean zone
- make canonicalization honestly fallible instead of panicking
- Commit current workspace changes
- verifiability says whether the closure is complete, not who checked the bytes
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- record why an observation failed, on both sides of "our failure"
- rename has_lease to leases_configured, and cut this session's comment bloat
- Binding gains bound_at_seq: a snapshot of the anchor at binding time
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Minimise Binding, demote bound_version to a store-layer view (RFC A2)
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- first commit

## [0.3.2](https://github.com/Anchorstate-Lab/GMR/compare/v0.3.1...v0.3.2) - 2026-08-14

### Added

- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(store,runtime)* [**breaking**] 写入令牌覆盖全部观测路径；Fence 用类型表达「没有令牌」
- *(store)* 退场不再清零令牌计数器
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- release v0.3.1 ([#5](https://github.com/Anchorstate-Lab/GMR/pull/5))
- release v0.3.0
- prove the shipped rung on the shipped schema, not only on a toy one
- decide the migration inside the write lock, not before it
- raise the schema to v7 for a per-anchor probe budget, and climb into it
- carry an older database across instead of refusing it
- apply busy_timeout via SqliteConnectOptions, not a migration query
- release v0.2.3 ([#3](https://github.com/Anchorstate-Lab/GMR/pull/3))
- gmr check: stop re-walking and re-parsing the whole repo per anchor
- release v0.2.1 ([#2](https://github.com/Anchorstate-Lab/GMR/pull/2))
- anchor its comments as memory, join the clean zone
- make canonicalization honestly fallible instead of panicking
- Commit current workspace changes
- verifiability says whether the closure is complete, not who checked the bytes
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- record why an observation failed, on both sides of "our failure"
- rename has_lease to leases_configured, and cut this session's comment bloat
- Binding gains bound_at_seq: a snapshot of the anchor at binding time
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Minimise Binding, demote bound_version to a store-layer view (RFC A2)
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- first commit

## [0.3.1](https://github.com/Anchorstate-Lab/GMR/compare/v0.3.0...v0.3.1) - 2026-08-13

### Added

- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(store,runtime)* [**breaking**] 写入令牌覆盖全部观测路径；Fence 用类型表达「没有令牌」
- *(store)* 退场不再清零令牌计数器
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- release v0.3.0
- prove the shipped rung on the shipped schema, not only on a toy one
- decide the migration inside the write lock, not before it
- raise the schema to v7 for a per-anchor probe budget, and climb into it
- carry an older database across instead of refusing it
- apply busy_timeout via SqliteConnectOptions, not a migration query
- release v0.2.3 ([#3](https://github.com/Anchorstate-Lab/GMR/pull/3))
- gmr check: stop re-walking and re-parsing the whole repo per anchor
- release v0.2.1 ([#2](https://github.com/Anchorstate-Lab/GMR/pull/2))
- anchor its comments as memory, join the clean zone
- make canonicalization honestly fallible instead of panicking
- Commit current workspace changes
- verifiability says whether the closure is complete, not who checked the bytes
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- record why an observation failed, on both sides of "our failure"
- rename has_lease to leases_configured, and cut this session's comment bloat
- Binding gains bound_at_seq: a snapshot of the anchor at binding time
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Minimise Binding, demote bound_version to a store-layer view (RFC A2)
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- first commit

## [0.3.0](https://github.com/Zongming-He/GMR/releases/tag/v0.3.0) - 2026-08-12

### Added

- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(store,runtime)* [**breaking**] 写入令牌覆盖全部观测路径；Fence 用类型表达「没有令牌」
- *(store)* 退场不再清零令牌计数器
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- prove the shipped rung on the shipped schema, not only on a toy one
- decide the migration inside the write lock, not before it
- raise the schema to v7 for a per-anchor probe budget, and climb into it
- carry an older database across instead of refusing it
- apply busy_timeout via SqliteConnectOptions, not a migration query
- release v0.2.3 ([#3](https://github.com/Zongming-He/GMR/pull/3))
- gmr check: stop re-walking and re-parsing the whole repo per anchor
- release v0.2.1 ([#2](https://github.com/Zongming-He/GMR/pull/2))
- anchor its comments as memory, join the clean zone
- make canonicalization honestly fallible instead of panicking
- Commit current workspace changes
- verifiability says whether the closure is complete, not who checked the bytes
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- record why an observation failed, on both sides of "our failure"
- rename has_lease to leases_configured, and cut this session's comment bloat
- Binding gains bound_at_seq: a snapshot of the anchor at binding time
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Minimise Binding, demote bound_version to a store-layer view (RFC A2)
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- first commit

## [0.2.3](https://github.com/Zongming-He/GMR/compare/v0.2.2...v0.2.3) - 2026-08-08

### Added

- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(store,runtime)* [**breaking**] 写入令牌覆盖全部观测路径；Fence 用类型表达「没有令牌」
- *(store)* 退场不再清零令牌计数器
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- gmr check: stop re-walking and re-parsing the whole repo per anchor
- release v0.2.1 ([#2](https://github.com/Zongming-He/GMR/pull/2))
- anchor its comments as memory, join the clean zone
- make canonicalization honestly fallible instead of panicking
- Commit current workspace changes
- verifiability says whether the closure is complete, not who checked the bytes
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- record why an observation failed, on both sides of "our failure"
- rename has_lease to leases_configured, and cut this session's comment bloat
- Binding gains bound_at_seq: a snapshot of the anchor at binding time
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Minimise Binding, demote bound_version to a store-layer view (RFC A2)
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- first commit

## [0.2.2](https://github.com/Zongming-He/GMR/compare/v0.2.1...v0.2.2) - 2026-08-08

### Added

- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(store,runtime)* [**breaking**] 写入令牌覆盖全部观测路径；Fence 用类型表达「没有令牌」
- *(store)* 退场不再清零令牌计数器
- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- anchor its comments as memory, join the clean zone
- make canonicalization honestly fallible instead of panicking
- Commit current workspace changes
- verifiability says whether the closure is complete, not who checked the bytes
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- record why an observation failed, on both sides of "our failure"
- rename has_lease to leases_configured, and cut this session's comment bloat
- Binding gains bound_at_seq: a snapshot of the anchor at binding time
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Minimise Binding, demote bound_version to a store-layer view (RFC A2)
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- Update English output
- first commit

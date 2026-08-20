# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0](https://github.com/Anchorstate-Lab/GMR/compare/v0.3.4...v0.4.0) - 2026-08-20

### Fixed

- *(runtime,cli)* [**breaking**] whether a record is still watched is a corpus fact, not a filter each verb picks

## [0.3.4](https://github.com/Anchorstate-Lab/GMR/compare/v0.3.3...v0.3.4) - 2026-08-18

### Added

- *(content)* [**breaking**] 一个可接入的记忆库是一个值，不是装配处的一段代码
- *(content)* [**breaking**] 声明是一个独立能力，而且是同步的
- *(content)* 发现契约进基底，实例留在域侧
- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- Merge branch 'fix/memory-layer-review'
- *(content)* [**breaking**] Declaring 回到它唯一的实现所在的那一层
- *(content)* [**breaking**] 记录与它的声明同一趟产出，名字和地址各归各位
- *(memory)* [**breaking**] 五个可能互相矛盾的 Option 收成一个 Grounding
- *(memory)* 按版本取回是能力，不是准入要求
- release v0.3.2 ([#7](https://github.com/Anchorstate-Lab/GMR/pull/7))
- release v0.3.1 ([#5](https://github.com/Anchorstate-Lab/GMR/pull/5))
- release v0.3.0
- release v0.2.3 ([#3](https://github.com/Anchorstate-Lab/GMR/pull/3))
- release v0.2.1 ([#2](https://github.com/Anchorstate-Lab/GMR/pull/2))
- give string_newtype!'s try_new a structured error, not String
- make canonicalization honestly fallible instead of panicking
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- extract gmr-content so a battery stops depending on the orchestrator
- count revisions by a ChangeKind enum, not by a bare string
- Phase 5 (A2): move Manifest/FileEntry/Platform out of gmr-core
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- first commit

## [0.3.3](https://github.com/Anchorstate-Lab/GMR/compare/v0.3.2...v0.3.3) - 2026-08-16

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- release v0.3.2 ([#7](https://github.com/Anchorstate-Lab/GMR/pull/7))
- release v0.3.1 ([#5](https://github.com/Anchorstate-Lab/GMR/pull/5))
- release v0.3.0
- release v0.2.3 ([#3](https://github.com/Anchorstate-Lab/GMR/pull/3))
- release v0.2.1 ([#2](https://github.com/Anchorstate-Lab/GMR/pull/2))
- give string_newtype!'s try_new a structured error, not String
- make canonicalization honestly fallible instead of panicking
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- extract gmr-content so a battery stops depending on the orchestrator
- count revisions by a ChangeKind enum, not by a bare string
- Phase 5 (A2): move Manifest/FileEntry/Platform out of gmr-core
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- first commit

## [0.3.2](https://github.com/Anchorstate-Lab/GMR/compare/v0.3.1...v0.3.2) - 2026-08-14

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- release v0.3.1 ([#5](https://github.com/Anchorstate-Lab/GMR/pull/5))
- release v0.3.0
- release v0.2.3 ([#3](https://github.com/Anchorstate-Lab/GMR/pull/3))
- release v0.2.1 ([#2](https://github.com/Anchorstate-Lab/GMR/pull/2))
- give string_newtype!'s try_new a structured error, not String
- make canonicalization honestly fallible instead of panicking
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- extract gmr-content so a battery stops depending on the orchestrator
- count revisions by a ChangeKind enum, not by a bare string
- Phase 5 (A2): move Manifest/FileEntry/Platform out of gmr-core
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- first commit

## [0.3.1](https://github.com/Anchorstate-Lab/GMR/compare/v0.3.0...v0.3.1) - 2026-08-13

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- release v0.3.0
- release v0.2.3 ([#3](https://github.com/Anchorstate-Lab/GMR/pull/3))
- release v0.2.1 ([#2](https://github.com/Anchorstate-Lab/GMR/pull/2))
- give string_newtype!'s try_new a structured error, not String
- make canonicalization honestly fallible instead of panicking
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- extract gmr-content so a battery stops depending on the orchestrator
- count revisions by a ChangeKind enum, not by a bare string
- Phase 5 (A2): move Manifest/FileEntry/Platform out of gmr-core
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- first commit

## [0.3.0](https://github.com/Zongming-He/GMR/releases/tag/v0.3.0) - 2026-08-12

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- release v0.2.3 ([#3](https://github.com/Zongming-He/GMR/pull/3))
- release v0.2.1 ([#2](https://github.com/Zongming-He/GMR/pull/2))
- give string_newtype!'s try_new a structured error, not String
- make canonicalization honestly fallible instead of panicking
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- extract gmr-content so a battery stops depending on the orchestrator
- count revisions by a ChangeKind enum, not by a bare string
- Phase 5 (A2): move Manifest/FileEntry/Platform out of gmr-core
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- first commit

## [0.2.3](https://github.com/Zongming-He/GMR/compare/v0.2.2...v0.2.3) - 2026-08-08

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- release v0.2.1 ([#2](https://github.com/Zongming-He/GMR/pull/2))
- give string_newtype!'s try_new a structured error, not String
- make canonicalization honestly fallible instead of panicking
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- extract gmr-content so a battery stops depending on the orchestrator
- count revisions by a ChangeKind enum, not by a bare string
- Phase 5 (A2): move Manifest/FileEntry/Platform out of gmr-core
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- first commit

## [0.2.2](https://github.com/Zongming-He/GMR/compare/v0.2.1...v0.2.2) - 2026-08-08

### Added

- *(runtime)* [**breaking**] 事件和状况分开摆 —— 游标只对日志里发生过的事有意义
- *(core,probe,shell,domain)* [**breaking**] 探针版本挣出来 —— artifact 内容寻址，声明与派生分家

### Fixed

- *(core,runtime)* 终结是日志里发生过的事，不是对最后一个状态的解释

### Other

- give string_newtype!'s try_new a structured error, not String
- make canonicalization honestly fallible instead of panicking
- the declaration slot holds a name; the transport answers what it stands for
- Move retain/cadence out of the sealed Anchor into a settings store
- extract gmr-content so a battery stops depending on the orchestrator
- count revisions by a ChangeKind enum, not by a bare string
- Phase 5 (A2): move Manifest/FileEntry/Platform out of gmr-core
- Phase 4 (B3/B5/C2/C3/C7/D4/D7): behavior fixes on the new service boundary
- Split LinkStore out of Binding (RFC A3)
- Split Sealer out of BindingStore (RFC A1)
- Update English output
- first commit

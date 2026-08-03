# RFC: 绑定层职责与解耦建议

作者: 自动草稿（请在提交前署名）
日期: 2026-08-03

**概要**

本 RFC 汇总了在现有实现中发现的绑定（Binding）相关耦合问题，提出把绑定职责收窄为“用指纹/标识把记忆（Ref）和锚点（AnchorKey）建立纯关系”的原则，并给出分步迁移建议与待团队决策项。

**目标原则**

- 绑定的唯一职责是关系：`Ref × {AnchorKey}`。绑定不应包含版本（bound_version）、时间、订阅游标、锚点代（generation），也不应包含记忆与记忆之间的关系（links）——这些都是与运行时行为或阅读逻辑紧耦合的信息，各自该有自己的归属。
- 订阅/消费、历史与可检索性应留在各自负责的子系统：订阅在消费方；历史在 provider（如 git）；密封/存证在日志/密封服务。
- 视图/报告层（read/edges）应把锚点状态（journal fold）与记忆视图（binding + provider fetch）并行合成，而不通过修改绑定实现副作用。

**已发现的问题（全盘汇总）**

1. 耦合 A — `bound_version` 住在 `Binding` 里
   - 现象：`read::fetch_memory` 把 provider 返回的版本与 `binding.bound_version` 比较并据此设置 `MemoryView.rewritten`。记忆内容变时会触发一条链，导致用户认为需要重绑。
   - 问题：版本/可检索性是内容侧的问题（provider 能否 `fetch_at`），不应把“必须重绑”的责任塞到绑定上。

2. 耦合 B — `AnchorKey` 同时承担“关注方向标识”和“日志主键”两重角色
   - 现象：`open --supersedes` 通过使用新的 `AnchorKey` 来开启新一代，并通过 `Anchor.supersedes` 记录旧代；所有绑定因此不再指向新锚点。
   - 问题：绑定应认“方向标识”（stable identifier）；换代逻辑如果被绑定语义耦合，会让绑定失效。当前实现选择了“旧代终结，新代显式接替”，这是一条明确代价的设计决策。
   - 备注：此问题涉及设计取舍（GMR.md §2 的代价选择），若要改成 `AnchorKey` + `Generation` 的模式，应单独作为架构原则讨论。

3. 耦合 C — `BindingStore` 兼职密封仓库
   - 现象：`BindingStore` trait 同时包含 `bind/bindings_on/binding_of` 与 `seal/sealed`。
   - 问题：`seal`/`sealed` 属于日志/修订存证的职责，不属于绑定存储本身；把它放在同一 trait 强制所有 binding 实现者承担额外责任。

4. 耦合 D — `links`（记忆间关系）住在 `Binding` 里
   - 现象：`Binding.links: Vec<Link>`（`crates/gmr-core/src/memory.rs:53-60`）把 `Ref → Ref` 的记忆间关系（例如“矛盾”“接替”）和 `Ref × AnchorKey` 的绑定关系塞进同一个结构体；`bind()`（`crates/gmr-runtime/src/bind.rs`）也把 `links` 当成绑定的一部分参数接收。
   - 问题：`read.rs::carry_linked`（`crates/gmr-runtime/src/read.rs:110-126`）消费 `links` 时全程只在 `Ref` 之间遍历，从未用到 `AnchorKey`，证明这是两种阶不同的关系。现状导致：想给一条记忆加一条链接，也必须重新声明一遍它的锚绑定；`Binding` 只增不改，链接因此没法独立追加或演进。

5. 其他运行时代码耦合点（概要）
   - `Runtime` 将锚点生命周期、绑定管理和视图合成放在同一个 service 中，导致控制流、错误处理和事务边界难以分离。
   - `read.rs` 既做 journal fold（历史）又做远端 IO（provider.fetch），混淆了“只读视图”与“可能失败的检索”两种语义。
   - `BindingStore` 的实现（`MemoryBindings::latest`）在内存/SQL 实现上内置了最新/去重策略，掩盖了调用方是否需要“去重”还是“历史”视图。

**建议变更（分级、按代价）**

下面把改动分级：优先级 1 为低风险可回退改动；优先级 2/3 为需要更高层决策的改动。

优先级 1（小，推荐先做）

A1. 把 `seal` / `sealed` 拆出为独立 trait（或移到 `Journal`）：
- 在 `crates/gmr-store/src/bindings.rs` 中新增 `trait Sealer { async fn seal(&self, bytes: &[u8]) -> ContentHash; async fn sealed(&self, addr: &ContentHash) -> Option<Vec<u8>>; }`。
- `BindingStore` 保持只做绑定相关 API。实现层（sqlite/testkit）按需实现两个 trait。
- 代价：小；影响点：`revise.rs`、`open.rs` 中的 `bindings.seal` 调用要改为 `sealer.seal`。

A2. 最小化 `Binding` 结构（降级 bound_version 为视图/事件元数据）：
- 在 `crates/gmr-core/src/memory.rs::Binding` 中将字段限定为 `reference: Ref`, `anchors: Vec<AnchorKey>`（`links` 随 A3 一起挪出去，不再留在 `Binding` 里）。
- 若要保留 bound_version 作为历史快照，则把它放在 `BindingStore` 返回的视图（例如 `BindingRecord`）或单独的 `BindingHistory` 表/接口中，而不是在核心类型中强制存在。
- 代价：小→中（需要改造 store 层与 read 层以兼容）。

A3. 把 `links` 从 `Binding` 中拆出为独立的 `LinkStore`：
- 新增 `trait LinkStore { async fn link(&self, from: &Ref, to: &Ref, kind: LinkKind) -> Result<(), StoreError>; async fn links_of(&self, r: &Ref) -> Result<Vec<Link>, StoreError>; }`（建议放在 `crates/gmr-store/src/links.rs`，与 `bindings.rs` 平级）。
- `bind()`（`crates/gmr-runtime/src/bind.rs`）不再接收 `links` 参数；新增独立的 `link()` 调用写入 `LinkStore`。
- `read.rs::carry_linked` 改为查询 `LinkStore::links_of`，不再从 `binding.links` 里取。
- 代价：小；影响点：`crates/gmr-core/src/memory.rs::Binding`（移除 `links` 字段）、`bind.rs`、`read.rs::carry_linked`、`testkit`/`sqlite` 的绑定实现。

优先级 2（中等，需要协调）

B1. 调整 `read`/`fetch_memory` 的责任分界：
- 现有的 `ContentProvider` 接口已提供 `fetch` / `fetch_at` 和 `Fetched { version, bytes }`。这里要做的不是新增接口，而是把版本比较与 `MemoryView.rewritten` 判定逻辑从 `read.rs` 中抽离出来，封装到 provider/内容检索层。
- `read.rs` 只把 `BindingStore` 返回的 binding view 按需并行传入内容检索层获取结果并合成视图，不修改 binding。
- 代价：中；效果：消除 read 对 binding 结构的写时依赖。

优先级 3（高，需修改规范/文档并获 owner 批准）

C1. 彻底移除 `bound_version` 与 `MemoryView.rewritten`（可选路径）
- 含义：GMR 不再在视图层尝试判断“记忆是否被改写”；这一职责交给 provider（用户通过 git log）或消费方主动用 `fetch` 检查。
- 代价：改动较大，丧失某些可观测性；需在团队达成共识并更新 `GMR.md` §6。

C2. 把 `AnchorKey` 与 `Generation` 分家（影响 journal 主键）
- 含义：`AnchorKey` 只表示关注点；`Generation`（或 explicit supersede record）表示第几代状态机实例。
- 一种可能的折中方案：不动 `AnchorKey` 作为 journal 主键的角色，给 `Anchor` 声明加一个域自定义的 `concerns: Vec<ConcernTag>` 字段；`Binding` 按标签匹配而不是按具体 `AnchorKey` 匹配；新一代开锚时显式声明它继承哪些标签（与 `supersedes` 同一次密封动作），让“是否延续”仍是一次显式、可审计的动作，只是把决策粒度从“每条记忆”降到“每个世代”，避免接替时要逐条记忆手动重绑。
- 代价：最大；会影响 journal 的索引、`open`/`supersede` 行为、以及消费方语义。直接触及 `GMR.md` §2 已经写清楚代价的设计决定，属于策略性选择，须作为独立架构决策提给 owner，不与 A1/A2/A3 同批次表决。

**迁移步骤（最小方案：先做 A1 + A2 + A3）**

1. API 变更：
   - 在 `crates/gmr-store/src/bindings.rs` 新增 `Sealer` trait。
   - 修改 `crates/gmr-store` 的实现（`sqlite`、`testkit`）以实现 `Sealer`。
   - 修改使用处（`revise.rs`、`open.rs`）从 `bindings.seal` → `sealer.seal`。

2. 变更类型：
   - 修改 `crates/gmr-core/src/memory.rs::Binding`：移除 `bound_version`；新增（如需）`BindingRecord` 作为 store 层返回的复合视图：
     ```rust
     pub struct BindingRecord {
         pub binding: Binding,
         pub bound_version: Option<Version>, // store 层视图字段
         pub created_at: Option<DateTime<Utc>>,
     }
     ```
   - 让 `BindingStore::bindings_on` 返回 `Vec<BindingRecord>` 或新增接口 `bindings_view_on`，以便 `read` 层并行拉取 provider 内容而不用改变核心类型。

3. 拆分 `links`（对应 A3）：
   - 新增 `LinkStore` trait 与 `crates/gmr-store/src/links.rs`；`Binding` 移除 `links` 字段。
   - `bind()` 不再接收 `links` 参数；新增独立的 `link()` 调用写入 `LinkStore`。
   - `read.rs::carry_linked` 改为查询 `LinkStore::links_of`。

4. 修改 `read.rs`：
   - `fetch_memory` 改为接收 `BindingRecord`，并把“版本比较 / fetch_at”逻辑从 `fetch_memory` 内联代码中抽成独立方法，围绕现有的 `ContentProvider`（`crates/gmr-runtime/src/content.rs`）组织，不需要新增接口。
   - 保持 `MemoryView` 的字段（`rewritten`/`content_at_bind`）作为视图级别字段，但它们来源于该方法的返回，而非核心 `Binding` 类型。

5. 适配实现与测试：
   - 更新 `crates/gmr-store/tests`、`crates/gmr-runtime/tests` 中依赖过旧 `Binding` 字段的测试。
   - 在 `testkit` 中保留 `MemoryBindings` 的行为，但把 `seal`/`sealed` 绑定到 `MemorySealer`，把 `links` 绑定到 `MemoryLinks`。

**影响与兼容性保证**

- 逐步迁移（先 A1/A2/A3）能保证对外 API 兼容：
  - 在短期内，`BindingStore` 仍可返回含 `bound_version` 的视图，避免 runtime 行为中断。
  - 长期目标是把 `bound_version` 的语义从“关系字段”剥离，变为“事件/视图/快照”字段。

**测试建议**

- 单元测试覆盖：
  - `Binding` 类型序列化/反序列化与 store 的存取（`testkit` 与 sqlite）。
  - `read.rs::fetch_memory` 在新接口下的行为（rewritten、retrievable、unavailable）。
- 集成测试：
  - 场景测试：错误改动回退（F1→F2→F2*），确认绑定不被强制重写。
  - 场景测试：正常演进 + Agent 更新记忆 M2，确认绑定更新由 Agent 显式触发。

**待团队决策（需要 owner/设计者明确）**

1. 是否接受把 `bound_version` 从 `Binding` 核心类型中剥离？（推荐：先降级为 store view 元数据）
2. 是否接受把 `links` 从 `Binding` 中拆出为独立的 `LinkStore`？（推荐：是——`links` 是 `Ref × Ref` 关系，跟绑定的 `Ref × AnchorKey` 阶不同，混在一起会导致加一条链接也要重新声明整条绑定）
3. 是否接受把 `seal`/`sealed` 拆出为独立 trait？（推荐：是）
4. 是否要彻底删除 `bound_version` 与 `MemoryView.rewritten`？（`GMR.md` §6 已把“按版本取回”列为记忆层对 provider 的强制要求，这不是单纯的代码清理，需先讨论是否修订 §6 本身）
5. 是否要把 `AnchorKey` 与 `Generation` 分离？（需高优先级决策；C2 里给了一种可能的折中方案，但仍需单独立项讨论）

**附：建议的 PR 拆分（最小化回退单元）**

PR-1: `Sealer` trait
- 新文件/修改： `crates/gmr-store/src/bindings.rs` + `crates/gmr-store/src/sqlite/bindings.rs` + `crates/gmr-store/src/testkit.rs`
- 修改调用点： `crates/gmr-runtime/src/revise.rs`、`open.rs`

PR-2: `LinkStore` 抽离（对应 A3）
- 新文件： `crates/gmr-store/src/links.rs`（+ sqlite/testkit 实现）
- 修改： `crates/gmr-core/src/memory.rs::Binding`（移除 `links`）、`crates/gmr-runtime/src/bind.rs`、`crates/gmr-runtime/src/read.rs::carry_linked`

PR-3: `Binding` 精简与 `BindingRecord` 引入
- 修改： `crates/gmr-core/src/memory.rs`、`crates/gmr-store` 接口类型签名变更（增量兼容地返回视图）
- 修改： `crates/gmr-runtime/src/read.rs::fetch_memory` 接口调整

PR-4: `read.rs::fetch_memory` 内部重构（不新增接口）
- `ContentProvider`/`Fetched` 已存在于 `crates/gmr-runtime/src/content.rs`，无需新增模块
- 修改： 把版本比较 / `rewritten` 判定逻辑从 `fetch_memory` 里拆成独立方法

PR-5（可选，大改）: 删除 `bound_version` / 分离 `generation`
- 文档更新： `docs/GMR.md`
- 大范围改造：journal index、open/supersede 行为、read/edges 的输出格式

---

请在此 RFC 下回复你的选择（对 1-5 的采纳意见），或把该草案转交 owner 以便召开设计审议。若同意，我可以根据选择立即生成 PR-1、PR-2 与 PR-3 的 patch 草案并运行相关测试。

```
flowchart TD
    Start["事件发生"] --> Kind{"这是哪一类事件？"}

    Kind -->|"调度/手动 observe(key)"| Fold1["fold(journal) 折算出当前 AnchorState"]
    Kind -->|"作者操作：对已有锚"| AuthorGuard{"s.closed ?"}
    Kind -->|"作者操作：开一个锚"| OpenGuard{"new_key 已经开过？"}
    Kind -->|"记忆操作 bind()"| BindPath["写一条新 Binding：ref + 同一批 anchors + bound_version"]

    %% ============ 观测驱动：状态机自己跑 ============
    Fold1 --> ClosedCheck{"closed ?"}
    ClosedCheck -->|"是"| Closed1["Observed::Closed<br/>不调用探针，不写任何日志"]
    ClosedCheck -->|"否"| Invoke["invoke：在 state.position 上调用探针→插件"]

    Invoke -->|"ProbeError：够不着/答非所问"| AttemptWorld["Entry::Attempt(Unreachable/Unusable)<br/>attempts+1，state 不动"]
    Invoke -->|"Sighted(outcome, derivation)"| Trans["transition：对当前 state 求值转换表"]

    Trans -->|"Unevaluable：判据本身炸了"| AttemptEval["Entry::Attempt(Unevaluable)<br/>我们的失败，不是世界的失败"]
    Trans -->|"Unchanged：没有规则命中"| SameState["next = state（值不变）"]
    Trans -->|"To(next)：某条规则命中"| NewState["next = 新状态值"]

    SameState --> StillCheck{"anchor.retain == Full ?"}
    NewState --> StillCheck

    StillCheck -->|"是"| WriteTrans["写 Entry::Transition<br/>（保留每一次观测，不折叠）"]
    StillCheck -->|"否，Tick"| SameAddr{"state 和 fact_address<br/>是否都跟上次相同？"}
    SameAddr -->|"是"| WriteStill["写 Entry::Still<br/>只挪 last_sighting，latest_seq 不动"]
    SameAddr -->|"否"| WriteTrans

    WriteTrans --> TerminalCheck{"新状态命中 terminal 集合？"}
    TerminalCheck -->|"是"| ClosedTrue["closed=true<br/>（自动终结，不需要 Close 条目）"]
    TerminalCheck -->|"否"| StaySame["锚继续跑，AnchorKey 不变"]

    WriteStill --> StaySame
    AttemptWorld --> StaySame
    AttemptEval --> StaySame

    %% ============ 开锚：新建 vs 换代 ============
    OpenGuard -->|"是"| OpenReject["拒绝：AlreadyOpen"]
    OpenGuard -->|"否"| SupersedeChoice{"这次 open 带 supersedes？"}

    SupersedeChoice -->|"不带，纯新开"| FreshOpen["invoke 一次拿初始 obs → transition<br/>Entry::Open，全新 AnchorKey，无血缘"]
    SupersedeChoice -->|"带，声明接替 old_key"| SupersedeCheck{"old_key 已经 closed？"}

    SupersedeCheck -->|"否"| SupersedeReject["拒绝：NotClosedYet<br/>不允许两代同时活着"]
    SupersedeCheck -->|"是"| NewAnchor["invoke 一次拿初始 obs → transition<br/>Entry::Open，新 AnchorKey<br/>Anchor.supersedes={old_key,rationale}<br/>旧锚永久保留，绑定不自动迁移"]

    FreshOpen --> StaySame
    NewAnchor --> StaySame

    %% ============ 作者驱动：改声明 / 主动关闭 ============
    AuthorGuard -->|"是"| Reject["拒绝：AnchorClosed"]
    AuthorGuard -->|"否"| AuthorKind{"revise 还是 close？"}

    AuthorKind -->|"revise(Reprobe/Retransition/<br/>Reterminal/Restate)"| Revise1["Entry::Revise<br/>改声明本身或直接改 state<br/>Restate 不移动 latest_seq"]
    AuthorKind -->|"close(rationale)"| Close1["Entry::Close"]

    Revise1 --> StaySame
    Close1 --> ClosedTrue

    %% ============ 记忆：与锚完全解耦的一条轴 ============
    BindPath --> BindNote["不要求 anchor 存在，也不要求它未终结"]
    BindNote --> ReadTime["read() 现算：<br/>rewritten = fetch(ref).version != bound_version<br/>（从不落盘，每次现读现算）"]
```
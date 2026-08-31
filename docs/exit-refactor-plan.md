# 出口重构实施计划

状态:**待审核。未获批准前不执行任何一步。**

把 CLI、SDK 与域的边界改成一条可被机器看守的判据。本文含全部决策台账、每步的完成定义与回滚路径。

---

## 1. 目标

今天 CLI 有约 31 个动词,其中 `read --json` 吐的类型不在契约里——而它正是任何第二个前端(hook、编辑器集成、投递层)唯一需要的那一次调用。SDK 有 7 个动词、有版本、有被 gate 看守的形状哈希。两边站在同一个运行时上,却只有一边被守着。

**这次重构不新增能力。** 三次查证都得出同一个结论:要的东西已经实现了,只是没有出口。所以主体是开门和命名。

| 已存在 | 在哪 | 为什么用不上 |
|---|---|---|
| `read --json` 带记忆全文 | `memories[].grounding.content` | 动词叫 "read an anchor",名字没说它是什么 |
| 便宜的 fold-only 读 | `Runtime::sample` / `read` / `read_all` | CLI 一个都没用,走的是带记忆抓取的路径 |
| 锚 key 的廉价列举 | `Runtime::anchors()`,9 个 verb 在内部用 | 没有任何出口 |

---

## 2. 这份计划为什么存在

上一轮 grilling 敲定了八个设计问题,细到契约闭包有几个类型。**没有一个问题是"每一步之后什么必须仍然成立"。** A→E 加提交粒度是顺序,不是安全——顺序只规定按什么次序去破坏。

实际发生的三件事,都不是判断错误,是**没查前提**:

- 读了 `LinkStore` 的 trait 就动手,**没读 `schema.rs`**——整库 append-only by trigger,`relink` 一写就撞上。
- 验证时看输出不看退出码,把两次静默失败的 `sync` 当成"幂等通过"。
- 为清理有界的错误(381 条链接)做了无界的操作(重建整个 store),而重建删掉了 `check` 一直依赖的队列残留行。

所以这份计划的每一步都必须回答五件事,少一件不执行:**做什么 · 前置 · 完成的定义 · 只读验证 · 回滚**。

---

## 3. 唯一的硬不变量

**这个仓库有两类东西,只有一类有 undo。**

| 类别 | 内容 | 回滚方式 |
|---|---|---|
| **可逆** | 源码 · 文档 · `memories/*.md` 文件 · `tools/gate.py` | `git`,一条命令 |
| **不可逆** | `.anchor/state/memory.db` —— journal · bindings · links · sealed | **没有。** schema 里逐表 `RAISE(ABORT,'append_only')` |

### 因此:本计划的 A–E 五步,没有任何一步写日志

**会写日志的动词**,不得作为验证手段出现:

```
sync · observe · check · pass · open · close · bind · said · accept · revise
```

**已验证的只读动词**(源码级确认:不调用任何写方法):

```
read · status · doctor · memories · cobound · health · edges
```

验证只用这七个,加 `cargo test` 与 `tools/gate.py`。

---

## 4. 决策台账

两轮 grilling 的全部结论,含被推翻的。**推翻的比留下的更值钱——它们标出了下一个人会踩的地方。**

### 第一轮 · 定位与出口

| # | 结论 | 状态 | 理由 |
|---|---|---|---|
| 1 | 顶层轴 = surface(用途) | **修订** | 改为 plumbing/porcelain × 两回路,两刀正交 |
| 2 | payload 是契约,动词名是 UI | 立 | 对标 `kubectl -o json`:`get` 是 UI,`v1.PodList` 是契约 |
| 3 | `gmr-surface` 是真实 crate | **撤回** | 契约已在 `gmr-runtime`,动词就是 `Runtime` 的方法;再包一层是第二个说 GMR 语义的地方 |
| 4 | 取消 `domains/` 改名 `shells/` | **撤回** | 规则 12(不可再议)写着 "domains own … assembly, and CLI";改名动宪法而什么都不买 |
| 5 | MCP:attest 面全给 + drift 面只读 | 立 | 工具表里没有的 agent 调不到——把 seal 从 prompt 约束变成结构约束 |
| 6 | 库优先 | **修订** | 改为 plumbing 优先;库只是 plumbing 的一种交付形态 |
| 7 | 三面切分 attest/drift/declare | **撤回** | 回到 `memories/three-layers.md` 的两回路 + 共用机制 |
| 8 | `--json` 一次性 breaking,payload 带版本 | 立 | agent 侧协议随二进制发布(`skill.rs` 的 `include_str!`),无长尾消费方 |
| 9 | Rust 走 git dependency 按 tag | 立 | 上 crates.io 要发 12 个包并永久占名,而此刻正在改契约 |
| 10 | 选锚边界不动 | 立 | GMR 不判语义真假、不替你选锚、不拦回答 |
| 11 | 注入动词是产品,hook 是参考实现 | **修订** | 主入口 `read` 已存在;要做的是补 `sample` 和改文档 |
| 12 | 一次性读数:有地址、可被 `saw` 引用、不建长期锚 | 立 | 服务侧动态事实源(过敏原场景)。**本轮不实施** |
| 13 | `since` 拆两半 | 立 | 带 status 0.18ms / 不带 7.23ms,签名看不出。**本轮不实施** |

### 第二轮 · 实施细节

| # | 结论 | 状态 | 理由 |
|---|---|---|---|
| 1 | 契约定义并强制 plumbing/porcelain | 立 | 不被检查的散文会漂——这一路已证明三次 |
| 2 | 只转 `read` 和 `open`;契约不为 CLI 便利扩容 | 立 | `bind --json` 的 `vouched` 是 `source.independent()` 的终端拼写,留在 porcelain |
| 3 | `sync` 把 wikilinks 写进 `LinkStore` | **已证伪** | 整库 append-only;派生缓存进证据库是范畴错误 |
| 4 | 加 `relink(from, kind, to)` | **已证伪** | 随 #3 一起废 |
| 5 | CLI 加 `sample` → `Vec<Reading>` | 立 | 已实现并实测:单文件 3.6×,全仓 3.3× |
| 6 | 不改名,只下沉可复用部分 | 立 | 规则 12 的 "batteries supply reusable implementations" 本来就准许 |
| 7 | 下沉 prose + shapes + coord + providers 声明类型 | 立 | 提示风险后由 owner 确认按全量执行 |
| 8 | MCP = `gmr mcp` 子命令 | 立 | MCP 服务"没有 vertical 可嵌"的场景 = 代码仓库 = coding 域 |

---

## 5. 步骤

四步,逐个提交,**全部只碰源码**。

---

### A · 契约 v9 与 plumbing 层 〔git 可逆〕〔已验证过一次〕

**做什么**

- 17 个类型进 `crates/gmr-runtime/src/contract.rs`(gmr-core 12 · gmr-runtime 4 · gmr-content 1):
  `Anchor` `AnchorView` `ContentErrorCode` `Facts` `FailureCode` `Faltering` `Grounded` `Link` `LinkKind` `MemoryView` `ProbeRef` `ReasonClass` `Rule` `Sighting` `State` `Superseded` `Transitions`
- `gate.py` 的 `CONTRACT_CRATES` 加 `gmr_content`;门面 `crates/gmr/src/lib.rs` 加 `Faltering`
- `CONTRACT` v8→v9,`SHAPE` 重算;`dist/npm/index.d.ts` / `index.js` 版本串跟改
- `open.rs` 改吐 `Opened`(`opened` 键改回 `key`)
- CLAUDE.md 新增 §11 定义 plumbing/porcelain 与动词名单
- `gate.py` 加 `check_plumbing_prints_contract_types`:从 §11 读名单,断言这些动词的 `println!` 路径不含 `json!`
- `Cargo.toml` 0.5.0 → 0.6.0(§10:契约破坏挣一个 minor,人工改)

**前置**:工作区干净(除会话开始就有的 `README.md` / `docs/GMR.md` / `SKILL.md`);`tools/gate.py` 当前为绿。

**完成的定义**

- `python3 tools/gate.py` exit 0,且新检查出现在清单里
- **新检查有牙**:临时把 `open.rs` 改回 `json!` 必须变红,恢复后变绿
- `cargo test -p gmr-runtime -p coding-anchor -p gmr` 全过(基线 340)
- `gmr read <任一坐标> --json` 输出与改动前逐字段一致

**只读验证**

```sh
python3 tools/gate.py; echo "gate=$?"
cargo test -p gmr-runtime -p coding-anchor -p gmr
./target/debug/gmr read "crates/gmr-runtime/src/read.rs#refresh" --json
./target/debug/gmr status | head
```

**回滚**:`git reset --keep <A 之前>`。不触及 `.anchor/`。

---

### B · CLI 加 `sample` 〔git 可逆〕〔已验证过一次〕

**做什么**

- 新 `verbs/sample.rs`:`gmr sample [coord] [--json] [--fresher-than-secs N]` → `Vec<Reading>`
- `render.rs` 加 `reading()`,复用已有的 `knowledge()`
- `cli.rs` / `lib.rs` / `verbs/mod.rs` 接线;CLAUDE.md §11 名单加 `sample`

零新契约类型——`Reading` 已在册,且正是 addon 的 `sample` 交回的东西。

**前置**:A 已提交且 gate 绿。

**完成的定义**

- gate 绿,包括对 `sample` 的新检查
- `cargo test -p coding-anchor` 全过(基线 185)
- **实测比 `read` 便宜**:同一坐标,`sample --json` 明显快于 `read --json`(上次实测 117ms vs 427ms)
- 不带 `--fresher-than-secs` 时不触发观测——比对前后 `sighting` 行数不变

**只读验证**

```sh
python3 tools/gate.py && cargo test -p coding-anchor
./target/debug/gmr sample "crates/gmr-runtime/src/read.rs" --json | head
time ./target/debug/gmr sample "crates/gmr-runtime/src/read.rs" --json >/dev/null
time ./target/debug/gmr read   "crates/gmr-runtime/src/read.rs" --json >/dev/null
```

**回滚**:`git reset --keep <B 之前>`。

---

### C · 链接与拓扑 —— 已删除,不执行 〔废弃〕

**原计划**:`sync` 把 `memories/` 的 390 条 `[[wikilink]]` 写进 `LinkStore`,并加 `relink` 让边可退休。

**为什么废**

- `schema.rs` 对 `links` 表有 `RAISE(ABORT,'append_only')`——`relink` 的 DELETE 被拒。整库如此,因为它装的是**证据**。
- wikilink 是从笔记正文**派生**的,每次 sync 重算,是缓存不是断言。缓存进证据库无法退休。
- `reaching` 报的是"我依赖的东西里哪些动了"——**依赖是断言**。拿引用填它会让它报告没人声明过的依赖。**`reach` 没坏,是 `gmr link` 没人用过。**

**留下什么**

拓扑仍然可建:`read --json` 已经交出每条绑定记忆的全文,`[[…]]` 就在里面。这正是"基础设计提供了,深度使用才能发现"。

建议把这条写成 `memories/` 里的一条记忆(锚在 `link.rs#reaching`),因为下一个人会重走这条弯路。**但写记忆要跑 `sync`,那是写日志——需单独批准。**

---

### D · 下沉可复用部分到 batteries 〔git 可逆〕〔最大的 diff〕

**做什么**

| 模块 | 行数 | 耦合 | 去向 |
|---|---|---|---|
| `prose.rs` | 220 | 零(只依赖 `pulldown_cmark`) | `batteries/prose` |
| `shapes.rs` | 1261 | 只有 `CliError` | `batteries/coord`,自带错误类型 |
| `coord.rs` | 268 | `CliError` + `Catalog` 类型 | `batteries/coord` + `Routes` trait |
| `providers.rs` 的 `Decl`/`Ids`/`Versioning` | ~50 | serde 声明类型,干净 | `batteries/provider` |

`Catalog` 调 `coding_extract::declares`,绑着 tree-sitter,搬不动——所以电池定 `Routes`(`for_extension` / `obs_of` / `kind_of` 三个方法),域的 `Catalog` 实现它。`coord` 的公开函数本来就把 catalog 当参数收,改动很小。

`providers.rs` 的 `declared()`(读 `providers.toml`)与 `assembled()`(装配)**留在域**——规则 12。

**前置**:A、B 已提交,gate 绿。契约已稳定——D 不碰任何契约类型,所以 `SHAPE` 不应变化。

**完成的定义**

- `SHAPE` 与 `CONTRACT` **一字不变**(D 若改了契约,说明搬错了东西)
- gate 绿,含 `check_layering` / `check_forbidden_dependencies` / `check_comments_clean`(新电池是干净区,零注释)
- 全量 `cargo test` 通过
- `tests/embeddable.rs` 仍然编译并通过——它是"这个域可以被别的前端调用"的凭证
- `gmr status` 与 `gmr read --json` 的输出与 D 之前逐字节一致

**只读验证**

```sh
# 先存基线,再改
./target/debug/gmr read --json > /tmp/before.json
grep -n 'SHAPE\|CONTRACT' crates/gmr-runtime/src/contract.rs > /tmp/before.contract

# 改完
python3 tools/gate.py && cargo test
diff <(./target/debug/gmr read --json) /tmp/before.json
diff <(grep -n 'SHAPE\|CONTRACT' crates/gmr-runtime/src/contract.rs) /tmp/before.contract
```

**回滚**:`git reset --keep <D 之前>`。建议按模块分四个提交,便于部分回退。

---

### E · `gmr mcp` 子命令 〔git 可逆〕

**做什么**

- `shells/mcp` 库 crate;`domains/coding/cli` 加 `gmr mcp` 子命令跑它,用域现有的装配
- 工具表 = **attest 面全给**(`sample` · `ground` · `bind` · `revoke`)**+ drift 面只读**(`since` · `status` · `check` · `doctor`)
- `anchor` · `sync` · `open` · `revise` · `accept` · `rebase` · `close` **不进工具表**——把 seal 从 prompt 约束变成结构约束

**前置**:A–D 完成。E 依赖 B 的 `sample` 存在。

**完成的定义**

- gate 与全量测试绿
- 工具表里没有任何写判据的动词——**写一条测试断言这一点**,否则下一次有人顺手加回来
- MCP 的响应形状是契约类型(与 `--json` 同一份)

**回滚**:`git reset --keep <E 之前>`。

---

## 6. 单列:上一次留下的残留

**F · 381 条 `cites` 链接与一个孤儿锚 〔触及日志〕〔需单独批准〕〔不含在 A–E 内〕**

**现状**

- `.anchor/state/memory.db` 里有 381 条 `kind='cites'` 的边。经 `carry_linked`,投递从 617 条涨到 2328 条(3.8×),涉及 659 个锚里的 531 个。
- 孤儿锚 `crates/gmr-runtime/src/read.rs#Blind`——为一条已删除的笔记开的,`doctor` 报 `undeclared`。

**可选路径**

- **留着。** 零风险,代价是投递永久带噪。
- **export → 过滤 → 新库 import → sync → 排空队列。** 已试过一次:能去掉链接,但会重建 `queue`,而 `check`/`observe` 依赖 closed 锚的残留队列行——上次正是它弄坏了 `check`。**若要走这条,必须先修 `check`/`observe` 让它们像 `status` 一样过滤 closed。**
- 孤儿锚:`gmr close … --why`(不可逆)或把笔记写回去。

**备份**

```
scratchpad/dbbackup/memory.db      手术前(= 当前生效的这份)
scratchpad/imported-rebuild.db     清掉链接那份(links 0,但 check 跑不了)
scratchpad/journal.jsonl           完整导出
scratchpad/clean.jsonl             已过滤 cites 的导出
```

### 顺带查出、尚未处理的两件事

- `check` 与 `observe` 无 key 时走 `rt.anchors()`,**包含 closed 锚**,而 closed 锚不入队 → 租不到就报"被别人占着"。今天能跑只因为旧库留着 closed 锚的队列行。同族的 `status` 明确过滤 closed。*这是既有缺陷,不是本次改动造成的。*
- `Blind::of` 把 `FailureCode::TimedOut` 映射为 `NeverAsked`,抢在 `ReasonClass` 分支之前。**不是 bug**——传输层用同一个码表示"探针跑了但超时"和"预算在调用前就耗尽",两者不可区分,`NeverAsked` 是 claim 更少的那一半。值得记成记忆。

---

## 7. 执行纪律

1. **动手前读那一层的不变量,不只是它的接口。** 上次读了 `LinkStore` 的 trait 就动手,没读 `schema.rs`。任何要写存储的步骤,先读 `schema.rs` 与该表的触发器。
2. **验证看退出码,不看输出。** 每条验证命令显式检查 `$?`。上次两次静默失败的 `sync` 被当成通过。
3. **只读动词做验证。** 七个已确认:`read` · `status` · `doctor` · `memories` · `cobound` · `health` · `edges`。`sync`/`observe`/`check`/`pass` 不得用作验证手段。
4. **改前存基线。** `read --json` 与 `SHAPE` 存文件,改后 `diff`。
5. **清理不得比错误更危险。** 若某步的回滚方案比该步本身影响面更大,停下来问,不要执行。
6. **每步一个提交,gate 与测试绿了才提交。** 红着不往下走。

---

## 8. 需要 owner 裁决的三处

1. **F 走哪条路。** 留着 / 做手术(需先修 `check`)/ 只处理孤儿锚。
2. **D 的电池划分。** 计划里是 `batteries/prose` 与 `batteries/coord`(装 coord + shapes + `Routes`)。也可以合成一块,或 shapes 单列。
3. **C 留下的那条记忆写不写。** 写它要跑 `sync`——那是写日志,按纪律需要单独批准。

# 涌现基底审计与修复指导(2026-09)

> 工作文档,非长期文档:指导本轮修复,修完即可删除,结论不锚定。
> 代码位置以行号引用,行号会漂移——修复时以符号名为准,本文只是当时的地图。

## 修复进度(2026-09-02)

已修:六个洞全部落地(契约 v11:lean 旋钮、said 可见 + condense 血缘、反向边 + at 列、
枚举上门、Fault kind+message;Usage 与 Ledger 两个可拒绝 store 能力,schema v14→v16),
清扫债 1-7 全部完成。

本轮**有意不做**、留待后续的:
- CLI 各动词不入走读账本(账本只在 console `served()` 咽喉计量;CLI 由 doctor 汇总读取)。
- transport 侧字节(五处 output_cap 已算出)未回传入账——回传链路侵入过深,首版只记信封字节+调用数。
- `since` 的 raised 隐藏 fetch 未改:status 过滤形态已是廉价路径。raised 也仍只报记录漂移,
  不报 said——话语没有会漂移的内容,其失效方式是 invariant 破裂,warrant 已经承载。
- `reach` 的 BFS 内部仍整取内容再弃(link.rs),lean 不改变 reach 的语义(Rewritten/NoBefore 区分依赖 history)。
- node `since` cursor >2^53 精度:JS number 边界,门层未拒绝,记录在案。
- 版本号:workspace 停留 0.6.x,major.minor 由 owner 亲手动;契约字符串独立于 crate 版本移动。

## 0. 判据

GMR 的定位赌注:相关性不由任何组件判断——GMR 不判断,Agent 侧也不需要专门的裁判——它在使用中从网络里长出来;Agent(哪怕弱模型)沿网行走,自行分析原子节点两两之间的关系,组装本次任务的最小信息集。最小信息集无法数学证明,但主张三个可测的不等式:比单点信息全面、比全量读取小、比自由搜索的有效信息密度高。

该赌注对基底提出四个可机械检查的前提,即本文全部修复的判据:

1. **可走性**:每一跳都存在且便宜,无断头路——一条断头路意味着网络的一整片区域对弱模型永久不可达;
2. **留痕性**:每一次使用都留下残渣,且留比不留便宜——"连接逐渐长成相关的样子"在机制上等价于这一条;
3. **原子性**:单个节点 + 单条边装进几百 token——弱模型每跳成本 O(1) 的前提;
4. **可测性**:三个不等式各自能变成一个可打印的数。

元检验:**若某修法需要 GMR 判断"这两个节点是否相关",该修法即错误。**

---

## 1. 审计一:行走(逐跳模拟弱模型代理)

### 动词面

- CLI(console/cli/src/cli.rs 定义,lib.rs:157-291 分发):32 个动词。
- SDK 门(console/node/src/lib.rs、console/python/src/lib.rs):仅 11 个——open(模块级)、ground、sample、read、since、bind、revoke、link、unlink、open(锚)、close。
- Runtime 有、两扇门都不外露的公开查询:`anchors()`(assembly.rs:45)、`read`/`read_all`(read.rs:566/572)、`grounded`/`grounded_all(_within)`(read.rs:594/789/793)、`cobound`(read.rs:785)、`links_of`/`all_links`/`reaching`(link.rs:38/42/46)、`claims()`(bind.rs:147)、`bindings_on`/`binding_of`(memory.rs:76/85)、`health`/`corpus`(health.rs:95/99)、`log().entries`(log.rs:30)。

### 关键跳的现状

| 跳 | 现状 |
|---|---|
| 坐标→锚键 | 仅 CLI:`resolve`(console/cli/src/verbs/mod.rs:67-133,精确键/前缀/path:line),全是 `pub(crate)`。门直接 `AnchorKey::new`,键不存在报 `NoSuchAnchor`(read.rs:1089)且无邻近提示 |
| 锚→接地信封 | CLI 与门都有,但信封臃肿(见审计三) |
| 锚→原子读数 | `sample` 两面都有,原子 ✓ |
| 记录→绑定于哪些锚 | **仅门**:`ground(["provider:id"])`;CLI 的 `gmr ground <id>` 硬编码 `Claim::said`(standing.rs:12),无法按地址接地存储记录 |
| 记录→出向链接 | 无独立动词,只作为锚信封中 MemoryView.links 字段出现 |
| 记录→**被指向的边(反向)** | **不存在**:LinkStore 仅 from 向(sqlite/links.rs:99-104 `WHERE l.from_ref = ?1`),无 trait 方法、无动词 |
| 记录→共锚兄弟 | 仅 CLI `cobound`,且把 said: 兄弟滤掉(cobound.rs:16 `filter_map(Claim::into_stored)`) |
| 多跳可达 | `ground` 的 `how.reach`(link.rs:46-102),上限 64,**只报 footing 非 Current 的节点**——是漂移巡检,不是邻域枚举 |
| 前缀/枚举 | 仅 CLI;门无 anchors()/claims()/grounded_all,`since(0)` 不能替代(只发射 Transition/Closed/Stalled,新开且安静的锚不可见) |

### 死路清单

1. 门侧无第一跳(坐标→锚)、无枚举——冷启动的 SDK 代理没有起点。
2. 反向链接不可查——`contradicts`/`supersedes` 这类语义恰恰在被指端最值钱,累积的连接从被指端永远发现不了。
3. 未绑定记录是链接黑洞:`carry_linked` 要求 stored+已绑定,否则静默跳过(memory.rs:264-267)。
4. said: 结论在信封里不可见:锚信封只收 `held()`(=stored)绑定(read.rs:1154-1156),`since` 的 raised 同样跳过(edges.rs:166)——站在锚上的代理看不见任何既有结论。
5. CLI 缺记录级 pivot(ground 硬编码 said)。
6. 唯一多跳动词按设计只报坏地基。

**判决**:今天弱模型仅靠门或仅靠 CLI 都走不完,两面各持半条走道、互补而不重叠。

---

## 2. 审计二:残渣(写路径)

### 存储面

Journal(append-only,触发器强制)/ BindingStore(append-only)/ Sealer / LinkStore(**签名 `link(from:&Ref, to:&Ref, kind, source)`,两端只能是 Ref**,links.rs:27)/ Queue(观察调度)/ Settings / Sightings。

**Sightings 定性**:每锚一行 `(count, last_at)`,写入者是 observe.rs:297(成功观察后)与 open.rs:162;**它是探针对世界的观察残留,不是记忆被使用的残留**——键是 AnchorKey,记忆(Ref)在 schema 里没有可被 sighted 的身份。

### 残留清单

- 留痕的:open / observe / said / bind / revoke / link / unlink / accept / revise / rebase / close——便宜、append-only、幂等,`gmr said "…" --on <锚>` 一条命令即可把小结论连 saw/depends 钉进网络。
- **零残留的**:sample / read / ground / since / cobound 纯读;`check` 算完 `handed` 只打印(check.rs:115-160),交付这个最该长边的信号一个字节不留;ground 的 reaching BFS 遍历后不留痕。
- **links 表没有时间列**(schema.rs:72-78,LinkRecord 同):一条边何时长出来不可读——"逐渐长成"这个过程本身不可观察。

### 结构性阻断

1. `Claim::Said` 无 Ref(gmr-core memory.rs:93-106)+ LinkStore 两端 Ref→Ref:**said 类型级地长不出边**;console 的拒绝(console/core/src/lib.rs:196-204)只是解释,不是关卡本身。
2. **无任何传承机制**:全仓无 promote/lineage/derive。结论落库(git/mem0)再 attest 后,新 Ref 白手起家——原 said 的 saw/depends 不迁移,link 又指不回 said:,方向、因果、时间全部丢失。
3. CLI link 硬编码 `Source::Adjudicated`(verbs/link.rs:44-49)且动词 `hide = true`。

### 摩擦评估

said 的仪式成本足够低;provenance 词表提供 unknown,来源声明永不成为写入阻力;criteria 变更(--why + 双封印)重且应当重。倒挂在读取侧:想留"我刚用了这条记忆"这种最廉价的残渣,**没有动词**——唯一出口 said 要求产出一段文字结论,不匹配该场景。

**判决**:写入半边成立,读取半边不成立——"关联性自然涌现"目前只对人工书写成立,对使用行为不成立。

---

## 3. 审计三:原子性与密度(实测)

实测方法:复刻 console/node/test/verbs.mjs 的最小世界(一锚 + 54B 记忆),经已构建 addon 实调计量;token 按 minified JSON ≈ 4B/token 估。

| verb | 一锚一忆实测 | 判定 |
|---|---|---|
| sample(Reading) | **440 B ≈ 150–190 tok**,零正文零探针 | **原子 ✓**——唯一守住"弱模型几百 token 持一节点"的读动词 |
| ground(Standing) | 640 B 起 | **臃肿**:`records_of`(read.rs:764-782)无条件 `provider.fetch` 回传记忆全文,而问题由 warrant+depends 已答完;本仓库记忆均值 3.2KB、最大 13.4KB → 真实一次 ground ≈ 1000–3700 tok |
| read(Grounded) | 1329 B 起 | **臃肿**:锚全定义(含 transition 源码+哈希)+ state + facts + 每条记忆全文;`Rewritten` 携带新旧**两份**全文(read.rs:180-186);carry:true 再拉一跳全文 |
| since(Edges) | 221 B/边 | 信封原子、**成本隐藏**:不带 status 过滤时每次调用为每条绑定记录完整 fetch 以算 raised(edges.rs:154-171);Edge::Transitioned 携带完整 from/to state,state 无上界则信封无上界 |

- `Instructions`(read.rs:21-41)只有 max_staleness/budget/reach/carry 四个旋钮,**无法表达"只要 warrant 不要正文"**。
- `sample` 默认纯折叠(journal + checkpoint,零 probe 零 fetch);但 `max_staleness` 一设即触发真探针调用(read.rs:617-629 → observe)。
- 做得好的:`Evidence` 只给地址与版本不给值(有测试断言);`Reached` 四字段。

### 测量仪器盘点

存在:latency.mjs(仅墙钟延迟,手动触发,bench.sh 路径已腐)、acceptance.py(仅正确性)、Sightings(探针计数)、gmr-budget(前瞻约束非账目)。
缺席:全仓无 ledger/telemetry/metric;没有任何东西记录一个会话消耗了几次调用、多少字节。**字节数每次都算过了又扔掉**——五个 transport 为 output_cap 都算了 size(http.rs:215、sql.rs:404、script.rs:78、shell/mod.rs:93、inproc.rs:130),`grounding_of` 手里有 `fetched.bytes`,无一记录。

### 账本提案(不建裁判)

走读账本一种行:{verb, 信封字节, 内容 fetch 次数/字节, 探针次数, 耗时},按 open 时铸的会话 id 分组。信封字节在唯一序列化咽喉 `console/core/src/lib.rs` 的 `answered()` 一行可得;transport 侧字节已算好只差写下。落点:Sightings 式的**可拒绝**新存储 trait(journal 记世界的事实史,调用方读了什么不是世界的事实,写进去污染折叠;Settings 是每锚配置,也不对)。三不等式各变一个数:比全量小 = 会话字节 ÷ 语料总量(今天 630KB/196 篇);比单点全面 = 每答案引用锚数(saw 在手);比乱搜密 = 被引用字节占走读字节比——`shown: seen/unseen` 已在用既有行裁定"取回的是否真被引用",这正是不需要裁判的原因。acceptance 加一条 guarantee:sample 信封 ≤ N 字节(今天 440B)。

---

## 4. 合成:六个洞

> 最锋利的一句:**今天这个系统里,相关性只能从人手里长出来,不能从使用中长出来。**

| # | 洞 | 破坏的前提 | 修向 | 归属 |
|---|---|---|---|---|
| 1 | **said: 结构性二等节点**:写不进边(LinkStore Ref→Ref、Said 无 Ref)、读不可见(信封/since/cobound 三处滤掉)、无凝结血缘通路 | 留痕 + 可走 | 要么给 Said 可入边身份,要么读侧可见 + 凝结动词(promote:新绑定携带原 said 的 saw/depends 与出身)| **owner(criteria 级)** |
| 2 | **使用零残渣**:读/交付不落库,Sightings 只数探针,links 无时间列 | 留痕 | 使用侧 Sightings 式可拒绝 trait(只计数不判断);links 补时间列 | 修 |
| 3 | **反向边不存在**:LinkStore 仅 from 向 | 可走 | trait 补 to 向查询 + 过门动词 | 修 |
| 4 | **门侧无第一跳/无枚举**:resolve 是 CLI 内部,anchors()/claims()/cobound/grounded_all 未上门;CLI 缺记录级 ground | 可走 | 该上门的上门(存在于 runtime,只是没接线);CLI ground 接受存储地址 | 修(动词表决定)|
| 5 | **两大信封臃肿**:ground/read 强制全文,Rewritten 双份,since 隐藏 fetch | 原子 | Instructions 加"按引用交付"旋钮(warrant 不带正文);是洞 6 的前置 | 修 |
| 6 | **密度不可测**:三不等式打印不出一个数 | 可测 | 走读账本(见 §3 提案)+ acceptance guarantee | 修 |

修复次序(按解锁涌现,不按工作量):**洞 1 先做决定 → 洞 5+3(小改,解锁行走与测量)→ 洞 2(生长引擎)→ 洞 6(变成数)→ 洞 4(补表)。**

---

## 5. 分支遗留(console-and-packs / PR #25 的清扫债)

| # | 债 | 修法 |
|---|---|---|
| 1 | console/node/bench.sh:9-10 仍拼 domains/ 旧路径,脚本已坏 | 改两行 |
| 2 | domains/coding/probes/test-roster.sh 仍在旧址被追踪,.anchor/probes.toml:11,13 指向它,且无任何记录说明为何留 | owner 决定:sync 走声明通道搬家并密封 why(script probe 身份是内容 hash,binding 不受伤),或写 memory 记录为何留 |
| 3 | console/python/gmr.pyi 声明 `from_` 而运行时参数是 `from`(Python 关键字):按 stub 关键字调用即 TypeError;typed-surface gate 只查 CONTRACT 字符串不查签名 | python 门参数改 from_,加关键字调用测试;gate 至少比对方法名清单 |
| 4 | bind/unlink 装配在 node 与 python 门各抄一份(~40 行逐句对应),含门内 `Utc::now()` 时钟读取、`asserted_as: None` 写死 | 下沉 gmr-console;时间作参数;暴露 asserted_as |
| 5 | Fault = String:库层字符串错误,测试 regex 匹配话术;SDK 调用者无法区分"不可达"与"不存在" | console 内 kind+message 结构,门映射 napi code / Python 异常子类;是否进 v11 是 owner 的 criteria 决定 |
| 6 | wheels 无 Windows 无 sdist;PyPI 无 token 时静默跳过,tag 存在而 PyPI 缺版本且无处可见 | 加 windows + sdist;未发布状态写进 GitHub release 正文 |
| 7 | docs/ARCHITECTURE.md:395,658,660 仍拼 domains/ | 改;并给 gate 加机械检查:非历史文件不得拼写 domains/ ——bench.sh 与文档是同类"清扫遗漏",该机械显形 |
| 8 | node since cursor 走 JS number → i64,>2^53 静默丢精度 | 记录即可(或门层拒绝超界) |

# GMR 的真实问题

这份文档只讲问题,不讲修法。每条都标了它是**实测**、**代码可查**,还是**推论**。

上一版按"价值为什么没落地"编排,把 agent 会不会主动调用当成根问题。那是消费层的事。这一版按**锚这个原语本身哪里不成立**重排,推翻的判断集中列在第九节。

---

## 零 · 判据

GMR 是**锚**:在推论和事实之间建立一条可被第三方重算的关系。

知识网络、最小信息集、"告诉 AI 该读什么"——这些是锚做好之后**挣来**的,不是要枚举的功能。没挣到,说明锚设计得不够好,不是功能不够全。

所以一条"问题"要进这份文档,得过这一关:

> **它是否让「锚」这个原语更能承载因果,或更能被廉价地问到?**

过不了的,是消费层的观察,放第八节,不作为待办。

三条不做,始终成立:不判断语义真假 · 不替你选锚 · 不拦截回答。这一版加第四条:**不计算最小信息集**——目标函数依赖任务,选集合时任务未知(论证见第十节)。GMR 欠的只有两件:让拓扑在坐标处可被廉价查询,和把边的类型如实交出去。

长期(coding 域)和短期(一次性回答)不是两个产品面,是同一个 δ 的两种消费深度。真正的分法在 [[three-layers]] 里已经写好,而且按**失效方式**分:fact 不失效 · memory 漂移 · inference 失去依据。

---

## 一 · 锚交出的东西,分不清「关于」和「被提到」

**这是根问题。** 锚就是"关于"这个关系;GMR 的主输出把它抹平了。

### 实测

`gmr read --json` 全量(release build,659 个锚):

```
                              条数    正文字节   每锚 p50   p90     max
声明的(warrant 有值)          615     2.7MB     3.4KB   8.6KB   14.6KB
顺着链接带进来的(无 warrant) 1713     7.6MB     8.5KB  28.6KB   76.3KB
                              ----    ------
交付占比                       26%       27%
```

**交出去的记忆里 74% 不是任何人声明过"关于这个坐标"的。**

单个坐标看得更清楚。`crates/gmr-runtime/src/read.rs#AnchorView`:

- frontmatter 里声明 `about:` 该坐标的:**3 条**(`cli-read-vs-status` · `render-diagnosis` · `runtime-read`)
- `gmr read` 交付:**17 条,88.5KB**

### 而区分它们的那个字段是反的(代码可查)

[memory.rs:149](crates/gmr-runtime/src/memory.rs#L149):

```rust
grounded: !bound.anchors().is_empty(),
```

`grounded` 的意思是"这条记录**在别处**绑过某个锚",不是"这条记录关于**这个**锚"。于是:

```
1713 条带进来的记录中,报 grounded: true 的:1713 条(100%)
```

真正的判别信号是 `warrant == None`,一个**缺省**。[runtime-read.md:25](memories/runtime-read.md#L25) 写明了这件事:

> `warrant` is `None` on exactly the records that were never bound to this anchor

但那是一条记忆里的句子,不是字段名。而名字叫 `grounded` 的那个字段,对全部 1713 条说 true。

### 人读的那一面完全没有区分(实测)

[render.rs:63](domains/coding/cli/src/render.rs#L63) 用 `m.grounded` 决定标记,`warranting(None)` 返回空串([render.rs:127-130](domains/coding/cli/src/render.rs#L127-L130))。所以 `gmr read <坐标>` 打出来是:

```
  * cli-read-vs-status  (rewritten since binding)
  * render-diagnosis
  * runtime-read
  * check-drift
  * runtime-ground
  … 另外 12 行,一模一样的 `*`
```

前三行是声明的,后十四行是"某条声明的记忆在散文里提到过它"。**输出里没有任何一处能把它们分开。**

### 为什么这条排第一

洗车店那条决定性信息之所以有用,是因为它**确实关于**这次决策。一个交付通道如果把"关于"和"提到"混在一起,给 AI 的就不是最小信息集,是一堆等权的候选——而注意力被稀释正是那个例子要解决的病。

上一版第二节测到"注入错的东西比不注入更糟"(B3 臂拿到旁支记忆后给出更差的方案)。**机制就在这里。**

---

## 二 · 边是从散文里编译进来的,而且冻住了

### 实测

```
memories/*.md 里的 [[wikilink]]     386 条(148 个源)
.anchor/state/memory.db links 表    381 条,kind 全部 = 'cites'
库里有、文件里没有                    3 条
文件里有、库里没有                    8 条
```

两张图对得上——库里那 381 条**就是** wikilink 图,由上一次失败的实施编译进去的。

三件事同时成立:

1. **语义被换掉了。** `[[x]]` 在 markdown 里的意思是"提到"。进了 `links` 表就成了 `kind='cites'`,而 [carry_linked, memory.rs:232-235](crates/gmr-runtime/src/memory.rs#L232-L235) 在交付时无条件跟随每一条,`kind` 直接丢弃:

   ```rust
   let linked: Vec<Ref> = memories
       .iter()
       .flat_map(|m| m.links.iter().map(|l| l.to.clone()))
       .collect();
   ```

2. **它是一份不可逆的拷贝,而且已经在漂。** [schema.rs:145-147](crates/gmr-store/src/sqlite/schema.rs#L145-L147) 对 `links` 建了 `no_update` / `no_delete` 两个 `RAISE(ABORT,'append_only')` 触发器。今天在笔记里加一个 `[[x]]` 或删一个,交付不变——库里那 381 条是某一次 sync 的快照。差异已经是 8 / 3。

3. **说这件事不对的那条记录自己没了。** `gmr doctor` 报:

   ```
   gone   link-mentions-are-not-dependencies, read-blind-timedout
          <- the provider says these records no longer exist
   ```

   `memories/link-mentions-are-not-dependencies.md` 在 git 的任何一次提交里都不存在,而它的 binding 永久留在库里。

这正是 [[three-layers]] 写下的那一条:*"a memory read out of the inference log is the checker checking against itself."* 只是这次落在 links 上,不是 `depends` 上。

### 基座在这件事上是对的,domain 没接

[runtime-carry-linked.md](memories/runtime-carry-linked.md) 明确写了边界:

> The walk stops at one hop on purpose … Deciding "is that distant memory still meaningfully about this anchor" is a judgment about **relevance**, and the substrate has no basis to make it; **it belongs to the domain**.

`LinkKind` 是 [`pub struct LinkKind(pub String)`](crates/gmr-core/src/memory.rs#L80)——开放词汇,符合核心规则 4;`MemoryView.links` 把 kind 原样交出。**基座把判断权和判断所需的信息都交出去了,domain 一层没有接:**[verbs/read.rs](domains/coding/cli/src/verbs/read.rs) 直接 `serde_json::to_string_pretty(&views)`,不按 kind 过滤,不按 warrant 分组。

所以这不是"基座缺功能",是**交付端从来没有行使过基座留给它的那个选择**。

---

## 三 · settled 不等于对

**580 / 600 个活锚是 settled(97%)。** 一个 agent 读到 settled,读成的是"这条记忆可信"。它实际的意思只有一个:**这个坐标上的代码没动过。**

这个边界本身是对的,[[gmr-not-entailment]] 论证过为什么必须如此:entailment 不可被第三方重算,把它放进基座会让整套可审计性架在唯一不可审计的部件上。问题不在边界,在**边界的后果没有出现在输出里**。

### 一个活的标本(实测)

`memories/runtime-carry-linked.md` 写着:

> `carry_linked` … and marks every one `grounded: false`

代码从不这样做。`grounded` 由 [`fetch_memory`](crates/gmr-runtime/src/memory.rs#L149) 写死为 `!bound.anchors().is_empty()`,`git log -S "grounded: false" -- crates/gmr-runtime/src/` **零命中**——这句话从写下的那天起就是错的。

而这条笔记的 frontmatter 是:

```yaml
about: crates/gmr-runtime/src/memory.rs#carry_linked
watch: [sig, logic]
```

它描述的行为在 `fetch_memory` 里,**一个它不看的符号**。`carry_linked` 的签名和逻辑确实没动过,所以这个锚 `settled`,`check` 一次都没把它交回来过。

**这个失效类语料自己已经写下来了**,在 [cli-read-vs-status.md:48](memories/cli-read-vs-status.md#L48):

> A note that quotes another layer's field names has to watch that layer, or it is grounded to the wrong thing and **the green means nothing**.

同一份语料里既有这条诊断,又有一个现成的病例。**GMR 按设计抓不到它**(那是 entailment),而唯一抓得到的办法——把笔记和代码逐条对读——恰恰是 GMR 存在的理由。

### 顺带的两个数(实测)

- 12 个锚 `moved`,`check` 只交回 **2** 条;`doctor` 另报 **15** 条 `quiet`——动了,但动的轴不在那条笔记的 `watch:` 里。**watch 的宽窄决定 88% 的移动是否被说出来(15 / 17)**,而它是手写的。
- 193 条笔记里 **22 条没有 `watch:`**,包括 `constitution` · `three-layers` · `gmr-not-entailment` · `anchor-RunSettings`。

---

## 四 · 定位不在锚里,不是够不着,是从来没锚过

### 实测:600 个活锚的落点

```
crates/ · domains/ · batteries/ · tools/   586   (97.7%)
CLAUDE.md                                    2
docs/GMR.md                                  2   ← 工作区已删除,未提交
docs/ARCHITECTURE.md                         0   ← 84KB,80 个小节,当前的架构文档
README.md                                    0   ← 11.8KB,面向用户的那一份
```

### 一个改代码的 agent 拿不到定位(实测)

四条"是什么"的记忆(`three-layers` · `gmr-not-entailment` · `constitution` · `runtime-aim`),对 586 个代码锚:

```
作为声明的绑定交付         4 个锚   0.7%   (全部是 runtime-aim,落在 health.rs 上)
只经由第二节那 381 条边到达 40 个锚   6.8%
永远拿不到                542 个锚  92.5%
```

`three-layers` 和 `gmr-not-entailment` **各只绑 1 个锚**,而那个锚是 `docs/GMR.md#…`。

### SSOT 换了,锚没跟着换(代码可查)

- `docs/ARCHITECTURE.md` 由 `ffce205` / `acb9b09` 引入,是当前的架构文档。**193 条笔记里 0 条提到它,0 个锚落在它上面。**
- `docs/GMR.md` 在**工作区**被删除,**未提交**——它仍在 HEAD 里,33KB。
- [CLAUDE.md:3](CLAUDE.md#L3) 写着 `*Architecture SSOT: GMR.md*`,[README.md:319](README.md#L319) 写着 `docs/GMR.md — architecture and design source of truth`。两处都还指着 HEAD 里那一份。

### GMR 抓到了(实测)

```
$ gmr check
docs/GMR.md#GMR 架构 > 0. 是什么   missing
  nothing there answered to any of file · heading
  → gmr-not-entailment
docs/GMR.md#GMR 架构 > 6. 记忆层   missing
  nothing there answered to any of file · heading
  → three-layers

2 of 659 handed a memory back. Re-read it: does what you wrote still hold?
```

**报得完全正确,而且报的就是那两条重心记忆。** 一次未提交的删除,系统立刻说出了后果。缺的不是检测,是把这句话送到正在动手的人面前。

**所以"定位够不着代码"不是拓扑距离问题。**距离和层级在这张图里正交,是因为定位**根本没有被锚在代码上**——586 个代码锚里没有一个声明过"这块东西为什么存在"。

---

## 五 · 每个 agent 走的那个出口不在契约里

[gate.py:320](tools/gate.py#L320) 定义得很清楚:*"The contract is whatever `contract.rs` re-exports — read, never restated."*

[contract.rs](crates/gmr-runtime/src/contract.rs) 重导出了 `Grounding` · `Warrant` · `Holding` · `Shown` · `Standing` · `Reading` · `Knowledge` · `Blind` …

**没有 `Grounded` · `AnchorView` · `MemoryView`。**

```
sample → Reading    SDK 用,目前没有已知真实用户    改字段 gate 就红
read   → Grounded   每一个 agent,SKILL.md 第 2 步   随便改,没人报警
```

叶子被守住了,信封没有。而 `Grounded { view: AnchorView, memories: Vec<MemoryView> }` 就是"原子答案锚定原子数据"的那个原子载体——第一节那个 `grounded` 字段正好住在这个洞里。

同层的两件:

- **CLI 有 94 处手写 `json!`,散在 24 个 verb 文件里**,没有版本标记、没有 schema、gate 一行不查。而 [tools/accept/driver.py](tools/accept/driver.py) 开头写着 *"No caller above this file may match on prose"*——验收套件把 CLI 的 `--json` 当契约用,那份契约没人守。
- `ContentErrorCode`([gmr-content/src/lib.rs:24](crates/gmr-content/src/lib.rs#L24))不在册,而 `Grounding::Unreachable` 和 `Before::Unreachable` 都在契约里、都带这个码。

---

## 六 · 问一个键要付全仓的价

实测(release build,659 锚 / 193 笔记):

```
gmr --version                          6 ms
gmr read   <一个坐标> --json         157 ms
gmr status <一个坐标> --json         559 ms      3.6×
gmr check  <一个坐标>                907 ms      5.8×
gmr read   --json  (全部)         21 393 ms
gmr status --json  (全部)         22 333 ms
gmr doctor                        23 991 ms
```

`status` 即使只问一个键,仍然无条件跑 [`memories::scan(root)`](domains/coding/cli/src/verbs/status.rs#L56)(读全部 193 条笔记)和 `sync::Bound::of(rt)`(投影全部 bindings)——那是三份 criteria 报告要的,单键调用方没要。**任何热路径前端只能建在 `read` 上。**

同族的三个动词对"哪些锚算数"意见不一致(代码可查):

```
status   filter(|g| named || !g.view.closed)   600 个   status.rs:50
read     不过滤                                659 个   read.rs
check    rt.anchors()                          659 个   check.rs:68
observe  rt.anchors()                          659 个   observe.rs:19
```

`check` 自己的输出就写着 `2 of 659`,而 `status` 数的是 600。59 个 closed 锚在两个动词眼里存在、在第三个眼里不存在。

**而 `doctor` 的报告是残缺的,它自己说了:**

```
unasked  25 of 217 bound record(s) were never asked about — the total content budget ran out first
         <- what is printed above is that partial view, not the whole repository
```

诚实,但截断由预算耗尽的**遍历顺序**决定,不由重要性决定——哪 25 条被丢没有任何语义。

---

## 七 · 三份 SKILL.md,没有共同身份

SKILL.md 是 Shape A 下唯一的协议载体。现在有三份:

```
.claude/skills/gmr/SKILL.md              == HEAD 的 assets/SKILL.md
domains/coding/cli/assets/SKILL.md       工作区已改回旧版(未提交,111+ / 169−)
npm 装的 gmr 0.5.0 里那一份              第三份
```

[skill.rs](domains/coding/cli/src/skill.rs) 用 `include_str!` 把 asset 编进二进制,`differs()` 拿它和磁盘上那份比。于是 `doctor` 报:

```
skill  /Users/zongming/Desktop/gmr/.claude/skills/gmr/SKILL.md
       <- this copy is not the SKILL.md in this binary … Delete it and re-run `gmr init`
```

**照它说的做,会把 HEAD 里那份好的覆盖成工作区里那份被改回去的。** 因为"正确"被定义成"你手上这个二进制里编进去的那份",而二进制来自一个未提交的工作区。

同一处的第二件:npm 装的 `gmr` 打不开这个仓库自己的库——

```
gmr: this database is stamped schema v12, and this build only knows v11. Refusing to open
```

两个二进制都自称 `gmr 0.5.0`。[`SCHEMA_VERSION`](crates/gmr-store/src/sqlite/schema.rs#L1) 是一个和 crate 版本无关的常量,**版本号不携带 store 世代**,所以"gmr 0.5.0"这句话不足以判断它能不能打开一个库。

---

## 八 · 消费层的观察(不在边界内)

以下全部是实测,但按第零节的判据,它们**不是 GMR 的待办**——它们是证据,说明第一到第四节那几条没修之前,锚交出的东西不足以在闭环成形之前改写问题。

AI 靠**最小逻辑闭环**结束任务:问"洗车店距我 50 米,走路还是开车",注意力落在 50 米 / 走路 / 开车,答"走路"就闭环了;补一条"99.9% 的人去洗车店是为了洗车",闭环被打破,答案才对。

给冷上下文 subagent 一个"看似合理但违反契约"的请求。裸组 = 仓库 + CLAUDE.md;注入组 = 额外给坐标本地的契约正文。

| 任务 | 裸 | 注入 | 效果 |
|---|---|---|---|
| A1 给 `Instructions` 加 `on_unreachable` | 照做 | **拒绝** | 决定性 |
| A2 把 `RunSettings` 移进 `Anchor` | 照做 | **拒绝** | 决定性 |
| B1 `ground` 加缓存 | 好答案 | 好答案 + 1 个正确性点 | 边际 |
| B2 语义检索 | 照做 | 照做 | 零 |
| B3 合并两个回路 | 照做 | **照做,且更糟** | **负** |

**A2 裸组是决定性的一例。** 它自己读到了 `memories/anchor-RunSettings.md`,引用了准入测试原句,写下 *"This contradicts an explicitly anchored decision… This is owner judgement (CLAUDE.md §7)"*,**然后照做了。**

```
裸组的闭环    「完成任务,顺带记下异议」   → 约束变成脚注
注入组的闭环  「这件事该不该做」          → 约束改写了问题
```

**同一条信息,闭环之前到达是重定向,之后到达是脚注。**

**B3 是负效应,而第一节解释了为什么。** 注入臂把注入的旁支契约当成了支持错误方案的论据:

> **The real prize:** … **Per `memories/check-drift.md`** … **that failure class disappears**.

那条记忆在拓扑上邻近、语义上相关,但因果方向上站在错误的一侧。**"邻近"不等于"构成最小信息集"**,而区分二者的信号(边的类型)在交付时被丢掉了。

**B3 的三臂对照最能说明第四节:**

| 臂 | 结果 | 理由的性质 |
|---|---|---|
| 裸 | 照做(变体) | **自己推出了核心论点**,归档成 caveat,方案仍从 store 读 |
| 注入坐标本地契约 | 全盘照做 | 无核心反对,还把注入的契约当论据 |
| 注入 + `three-layers` | **拒绝** | 正确理由,替代方案保住了性质 |

裸臂原话:*"check stops reading the memory. It would compare code against a copy in the append-only log."* **它发现了,然后按错误的重心把发现归档成了脚注。**

定位臂列了三条机制障碍,裸臂逐条工程绕过去了。**机制障碍可以被工程掉,层级区分不能。**

---

## 九 · 上一版里被推翻的判断

1. **"靠 agent 自觉的注入是根问题"** → 不是 GMR 的问题。CLI / MCP / SDK / hook 那张强度表是一张**关于 harness 的表**;基座设计得再对,四栏的差异原样存在。按 CLAUDE.md 规则 12,组装和 CLI 属于 domain。降级为第八节的观察。
2. **"语料的链接网络指向错误方向"** → 量的是 atlas 渲染用的 wikilink 图。它确实被上次失败的实施编译进了运行时,但**语义在编译中被换掉了**(提到 → cites → 交付时跟随),而且冻结了。真正的问题是第二节,不是出链分布。
3. **"定位从来没有被写进仓库"** → 写过,而且比上一版第零节写得好([[three-layers]] / [[gmr-not-entailment]])。按"没写"这个诊断走下去,结论是"再写一份文档",而那正是刚刚失败过一次的路径。真正的问题是**它没有被锚在代码上**(第四节)。
4. **"`docs/GMR.md` 已被删除"** → 工作区删除,**未提交**;HEAD 里还在,33KB。
5. **"最小信息量是否可计算"从"未解"移到"边界"** → 上一版的论证是对的(目标函数依赖任务,选集合时任务未知),所以这不是待答问题,是第零节的第四条"不做"。

---

## 十 · 这次调查自身的缺陷

1. **第八节的"裸"组不是裸的。** CLAUDE.md 自动进每个 agent 的上下文。实测的是「CLAUDE.md」vs「CLAUDE.md + 坐标本地记忆」。
2. **B2 证明不了任何事。** "设计语义检索"没有无歧义的错误答案,而且提示词是我写的——我自己把"记忆系统"的框架塞了进去,再测它们会不会偏向记忆系统。
3. **语料对搜索是可发现的。** `memories/three-layers.md` 是仓库里的文件,agent 能 grep 到。"锚定拓扑够不着"和"搜索找不到"是两件事,只区分开了一半。
4. **第一节的判别用的是 `warrant` 有无。** 这依据 [runtime-read.md:25](memories/runtime-read.md#L25) 和 `ground()` 的代码路径,不是独立观测。
5. **第六节的时间是单机单次(min of 5 / min of 2)**,不是基准套件。

---

## 十一 · 还没有答案的

- **宪法层能压到多小而不失去打断闭环的能力。** 那四条重心记忆 12.6KB,挂在每次编辑上太贵。可实验:注入 1KB 蒸馏版 vs 12.6KB 原文,比较打断率。
- **服务侧动态发现的事实源怎么锚。** 过敏原例子:AI 在分析配料时才发现要查"花生油是否致敏"。预先枚举 = 退回传统系统;运行时开长期锚 = 锚随对话无限增长;一次性读数 = 今天不存在。
- **注入在长会话中是否随上下文增长而失效。** 没测过。
- **第三节那类错误能不能被机械发现。** "笔记引用了它不 watch 的符号的行为"——这是 entailment,基座不能做。但一条笔记提到的**符号名**是否都在它的 `about:` 覆盖范围内,是结构问题,可算。不知道召回率有多低。

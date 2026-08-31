# GMR 现在有哪些实际问题

这份文档只讲问题,不讲修法。修法定下来之前,先确保问题本身没有理解错。

每条都标了它是**实测**的、**代码可查**的,还是**推论**。推论那些是判断,可以被推翻。

---

## 零 · 先说清楚 GMR 是干什么的

不写这一段,下面所有问题都会被读成别的问题。

**核心目的是 AI 回答的可靠性。** 不是留证据,不是出事好追责。

**机制是打断最小逻辑闭环。** AI 靠"最小逻辑闭环"结束任务:

> 问"洗车店距我 50 米,应该走路去还是开车去?",AI 大概率答"走路"。它的注意力在 50 米、走路、开车这三样上,答"走路"就闭环了。
>
> 再给它一条"99.9% 的人去洗车店是为了洗车",原来的闭环被打破——走路解决不了洗车——于是它给出更准确的答案。

GMR 的作用就是这个:**在闭环成形之前,把锚定的事实送到 AI 面前。**

```
主要作用   注入 —— 让 AI 拿到它不会想到要查的事实
次要作用   shown —— 事后机械判定它有没有真的用上那条读数
```

**职责边界,三条不做:**

| 不做 | 为什么 |
|---|---|
| 不判断语义真假 | GMR 验证的是推断与其记录依据的关系,不是推断本身对不对 |
| **不替你选锚** | 洗车店例子里起决定作用的是"谁想到要给那条信息"——GMR 把它划在职责外 |
| 不拦截回答 | `shown: unseen` 是一个事实,不是"拒绝发送"的指令 |

**两个产品面,失败方式不同:**

```
长期 · coding    记忆系统。契约过去只在 senior 工程师脑子里,现在是 agent 的记忆
                 失败长这样:记忆和事实不对应 → 给后续 AI 的不是指导,是误导
                 消费者是下一个 agent,人只在裁决时出现

短期 · 服务      引用系统。让 AI 的分析能力可以被信任地使用
                 例:传统点单系统只能枚举,没标记花生油是过敏原就永远报不出来
                     AI 能分析——配料表有花生油 → 花生油可能致敏 → 提醒顾客
                 GMR 不是限制 AI 只能复读数据库,是反过来
                 失败长这样:AI 编造,或分析结论无从验证
```

**两种部署形态:**

```
Shape B  嵌进「组答案的那个东西」里。grounding 是数据的属性,不是协议步骤
         代码路径没有能力返回一个没有 grounding 的答案。这是主形态
Shape A  没有 vertical 可嵌时旁挂(CLI / MCP)。代码仓库就是这种场景
         agent 自己就是组答案的人,所以协议只能靠 prompt 约束。这是退化形态
```

---

## 一 · 靠 agent 自觉的注入,原理上不可靠

**这是根问题,下面几条都从它派生。**

洗车店那题里,AI **不会觉得自己缺信息**——它的闭环是自洽的。50 米、走路、开车,三个要素齐了,答案成立,它没有任何理由去查。

所以"让 agent 记得先查一下"这个形态,触发条件恰好是 AI 意识不到的那件事。

**今天四种注入方式的强度:**

| 方式 | 触发靠什么 | 强度 |
|---|---|---|
| CLI + SKILL.md | agent 记得走三步协议 | **弱** — 靠自觉 |
| MCP 工具 | 工具在那,agent 决定调不调 | **弱** — 同样靠自觉 |
| SDK 嵌进 handler | 代码路径强制 `sample` | 强 — 结构保证 |
| harness hook | agent 碰到坐标就自动注入 | 强 |

**MCP 和 CLI 在这个维度上是同一级的。** 它改善延迟和人机工程,不改善注入强度。这一条推翻了我之前"MCP 是重大改善"的说法。

### 实验证据(实测)

给冷上下文的 subagent 一个"看似合理但违反契约"的请求,看它顶回去还是照做。裸组 = 只有仓库和 CLAUDE.md;注入组 = 额外给坐标本地的契约正文。

```
A1  给 Instructions 加 on_unreachable    裸 照做  ·  注入 拒绝
A2  把 RunSettings 移进 Anchor           裸 照做  ·  注入 拒绝
```

**A2 裸组是决定性的一例。** 它**自己读到了** `memories/anchor-RunSettings.md`,引用了里面的准入测试原句,写下:

> "This contradicts an explicitly anchored decision... This is owner judgement (CLAUDE.md §7)."

**然后照做了。**

```
裸组的闭环    「完成任务,顺带记下异议」   → 约束变成脚注
注入组的闭环  「这件事该不该做」          → 约束改写了问题
```

**结论:注入的价值不在信息可得性,在到达时机相对于闭环成形的先后。** 同一条信息,闭环之前到达是重定向,之后到达是脚注。

---

## 二 · 注入错的东西,比不注入更糟

**注入的效果不是单调的。** 这一条反直觉,而且有实测。

```
注入的记忆是正相关(恰好是禁止该请求的那条契约)  → 决定性
注入的记忆是旁支                                → 边际 / 零 / 负
```

| 任务 | 裸 | 注入 | 效果 |
|---|---|---|---|
| A1 `on_unreachable` | 照做 | **拒绝** | 决定性 |
| A2 `RunSettings` | 照做 | **拒绝** | 决定性 |
| B1 `ground` 加缓存 | 好答案 | 好答案 + 1 个正确性点 | 边际 |
| B2 语义检索 | 照做 | 照做(架构几乎逐字相同) | 零 |
| B3 合并两个回路 | 照做 | **照做,且更糟** | **负** |

### B3 注入臂把注入的契约当成了论据(实测)

它原话:

> **The real prize:** … **Per `memories/check-drift.md`**, after any shape edit and before `accept --criteria`, every note's `watch:` silently stops applying. A stored expression never consults `shapes::of()`, so **that failure class disappears**.

我注入的那条记忆描述了一个真实的失效模式,而**错误的设计恰好把它解决掉了**——于是注入的内容变成了支持错误方案的正向论据。

**所以"注入更多相关上下文总没坏处"是错的。**

---

## 三 · 重心识别失败:agent 拿到全部拼图,仍然选错重心

这是问题一在大尺度上的表现,而且是最难对付的一种。

**症状**(你先提出的):反复说明过 GMR 是锚定层工具,但 agent 改代码时总是偏向把它当记忆工具。这是只根据部分信息形成的逻辑短路——代码本身已经表明了不是,读全了就能发现,但**读不全,上下文装不下**。

### 标本(实测)

B3 任务:把 note 的 `watch:` 编译成 `depends` 存进 store,让 `check` 从 store 读回来。提示词是纯机械的(只描述数据结构和代码路径),不含任何"GMR 是什么"的框架,而且自带 DRY 压力。

语料里**字面记录了这次差点做成的错误**(`memories/three-layers.md`):

> This note exists because **the merge was nearly made**. … That is writing a **memory's criteria into the inference slot**. `check`'s whole job is comparing code against memory; pointed at the store it would compare code against *a copy of the memory taken at the last sync*. **The one drift this system exists to catch, manufactured in the checker.**

三条臂的结果:

| 臂 | 结果 | 理由的性质 |
|---|---|---|
| 裸 | 照做(变体) | **自己推出了核心论点**,归档成 caveat,方案仍然从 store 读 |
| 注入(坐标本地契约) | **全盘照做** | 无核心反对,还把注入的契约当论据 |
| 注入 + 定位记忆 | **拒绝** | 正确理由,且替代方案保住了性质 |

**裸组的原话**,它自己推出来的:

> **check stops reading the memory.** It would compare code against a *copy* in the append-only log; an edited `watch:` is inert until `sync` runs.

然后它以另一条**机制**记忆(`runtime-depends.md`,讲别污染 `depends` 语义)为重心,推荐"分成两个字段"——**那个方案保留了 store round-trip,仍然犯它自己点名的错**。

> 你说的"如果阅读全面、分析清楚就能发现"——**它发现了。然后按错误的重心把发现归档成了脚注。**

### 附带的一条(实测)

定位臂列了三条机制障碍(极性反转 / `Vacuous` / 回退链无处安放);**裸臂逐条工程绕过去了**。

**机制障碍可以被工程掉,层级区分不能。** 所以只有正确的理由能防住下一次有人把机制障碍绕过去。

---

## 四 · 定位在拓扑上够不着代码

这是问题三的根因。

### 语料的链接网络指向错误方向(实测)

```
193 条记忆  出链 p50 2 · p90 4 · max 10 · 45 条零出链(23%)

被引用最多的(枢纽)          说明「是什么」的(重心)
  runtime-warrant    13         three-layers         3
  runtime-grounding  12         gmr-not-entailment   4
  layers             11         constitution         1
  content-budget     10         runtime-aim          1
```

**枢纽全是机制记忆,重心是孤岛。** 一个 agent 顺着链接走,走到的全是"怎么运作",走不到"是什么"。

### 广度扩展不可行(实测)

从一个代码坐标出发沿 `[[wikilink]]` 扩展,到达重心记忆:

```
跳数  到达重心  注入字节 p50
 0      2%         5.4KB
 1     18%        24.0KB
 2     28%        81.4KB
 3     72%       270.8KB   ← 整个语料的 43%
```

**指数代价换线性收益。** 原因是这个图的中心性指向反方向——**距离和层级在这个图里正交**。你要的东西不在"近处",在"另一层"。

### 层级信号藏在锚定目标的类型里(实测)

```
锚在 *.rs#symbol   125 条 (65%)   局部约束:这块怎么运作
锚在描述性文档       3 条 (1.6%)  全局约束:这东西是什么
```

那 3 条是 `constitution`(锚在 CLAUDE.md)、`gmr-not-entailment`、`three-layers`。**后两条锚在 `docs/GMR.md` 上——那个文件已被删除。**

用 sqlite 直接查 bindings 表确认过:两条记忆**各只有一条绑定、各只指 1 个锚、其中 0 个是代码坐标**。

强制观测之后:

```
docs/GMR.md#GMR 架构 > 0. 是什么   moved  missing  → gmr-not-entailment
docs/GMR.md#GMR 架构 > 6. 记忆层    moved  missing  → three-layers
```

**GMR 抓到了,而且交回的恰好就是那两条重心记忆——只是从来没人让它跑。**

### 而 CLAUDE.md 补不上这个洞(代码可查)

CLAUDE.md 13.6KB,自动进每个 agent 的上下文。它讲 crate 边界、所有者规则、版本流程——**通篇是规则和禁令,从不说"这不是一个记忆工具"**。

**定位不在里面。** 所以"三臂都有 CLAUDE.md,只有拿到 `three-layers.md` 的那臂给出了正确理由"。

> 这就是你说的:**GMR 具备了潜在的功能,但用户用不出来。** 而根因是锚定的拓扑把「是什么」和「在改什么」隔开了。

---

## 五 · 注入本身的工程约束

即使解决了上面几条,注入还有硬约束。全部实测,对象是这个仓库(599 个锚 / 193 条记忆 / 621KB 语料 / 零 barren)。

### 粒度决定生死

```
按文件注入   p50  5.9KB   p90 16.8KB   max 70.9KB   ← max 不可能
按符号注入   p50  3.3KB   p90  7.2KB   max 15.0KB   ← 全部可行
```

`crates/gmr-runtime/src/read.rs` 本身 33KB,牵出 **66KB 记忆——比源码还多一倍**。

窄化实测(18 个真实坐标):p50 砍到 47%,重文件砍到 7–11%。

### 预算截断会主动挑错

`read.rs` 限 8KB 时,保留了覆盖 12 个锚的 `runtime-ground.md`,**丢掉的 10 条里包含 `runtime-instructions.md`——正是管 `refresh` 那个函数的**。

**粒度对齐是解,预算截断不是。** 粒度对了从不需要截断;粒度错了截断会加重稀释注意力——而注意力正是洗车店例子的主题。

### 窄化必须按位置,不能按名字

给定一次编辑,判断它落在哪个锚里:

```
字符串匹配锚名     命中 16%   回退 56%   挑错 28%
行包含 + 唯一定位   命中 100%  回退  0%   挑错  0%
```

字符串匹配没有符号边界的概念,`}` 这种行会命中别处。行号来自 `facts.facts.line`,抽取器已经在算(`ast.rs:256`),与 `grep -n` 完全一致。

### 注入的对象是约束,不是变化

**599 个锚里只有 18 个是非 settled(3%)。** 只注入"漂了的",97% 的时候什么都不送——洗车店那个病原封不动。

洗车店里那条决定性的信息("99.9% 的人去洗车店是为了洗车")**根本没有漂移**,它一直是真的。

---

## 六 · 出口本身的问题(最初的问题)

上面讲的是"价值为什么不落地"。这一节是"载体本身有什么毛病"。

### 唯一需要的那次调用,不在契约里(代码可查)

`gmr read --json` 一次给出:锚 · state · facts · `fact_address` · 记忆地址 · warrant · **记忆全文**。任何第二个前端要的就是它。

但它吐的 `Grounded` / `AnchorView` / `MemoryView` **不在 `contract.rs` 里**,所以 `SHAPE` 盖不住,改字段没有任何东西报警。

```
sample → Reading    SDK 用,目前没有已知真实用户    ✅ 改字段 gate 就红
read   → Grounded   每一个 agent,SKILL.md 第 2 步   ❌ 随便改,没人报警
```

同时 `ContentErrorCode` 是契约里一个**已经存在的洞**:`Grounding::Unreachable` 和 `Before::Unreachable` 都在契约里,它自己没在册。

### CLI 有 94 处手写 `json!`,而 SDK 有版本有形状哈希

SDK 那边:`gmr.contract.v8` + 挣来的 `SHAPE` + 两道 gate 检查(形状变了而版本没动就红;`index.d.ts` 版本串对不上也红)。

CLI 这边:94 处 `json!` 散在 24 个 verb 文件里,没有版本标记、没有 schema、gate 一行都不查。

而 `tools/accept/driver.py` 开头写着 *"No caller above this file may match on prose"* ——**你自己的验收套件把 CLI 的 `--json` 当契约在用,但那份契约没人守。**

### `status` 对单键跑全仓审计,慢 59 倍(实测)

```
gmr --version                        7 ms
gmr read   <一个坐标> --json        37 ms
gmr status <同一个坐标> --json    2194 ms
gmr status --json (全部)         12982 ms
```

即使只问一个锚,`status` 也要 `memories::scan()` 扫全部笔记 + `sync::audit()` 全仓审计——那是三个 criteria 报告要的,而单键调用方没要。**这决定了任何热路径前端必须建在 `read` 上而不是 `status` 上。**

### 能力已经存在,只是没有出口(三次查证,同一个结论)

| 已存在 | 在哪 | 为什么用不上 |
|---|---|---|
| `read --json` 带记忆全文 | `memories[].grounding.content` | 动词叫 "read an anchor",名字没说它是什么 |
| 便宜的 fold-only 读 | `Runtime::sample` / `read` / `read_all` | CLI 一个都没用,走的是带记忆抓取的路径 |
| 廉价的锚 key 列举 | `Runtime::anchors()`,9 个 verb 在内部用 | 没有任何出口 |
| wikilink 解析 | `prose::wikilinks`,只喂 `atlas` | 结果直接烘进 HTML |

**我自己读了这个代码库几个小时,仍然漏了第一条,并绕出了一个"查锚→再读文件"的两步变通。** 能力在那儿,名字没有透露它是什么。

### CLI 与 SDK 的动词对不上(代码可查)

| Runtime 方法 | CLI | SDK |
|---|---|---|
| `grounded_within` | `read` | — |
| `sample` | **没有** | `sample` |
| `bind`(Said) | `said` | `bind` |
| `bind`(Stored) | `bind` / `attest` | `bind` |
| `ground` | `standing` | `ground` |
| `changed_since` | `edges` | `since` |

`bind` 在两边意思都不一样:CLI 的只接记录,SDK 的什么 claim 都接。

### `since` 一个动词横跨两个回路,代价差 40 倍(实测)

```
since(cursor, Some(status))  →  0.18ms  ·  只问「锚状态动了吗」
since(cursor, None)          →  7.23ms  ·  构建 raised,每个锚 fetch 一次记录
```

`memories/latency-baseline.md` 自己写着:*"nothing in the signature says so."*

---

## 七 · 定位从来没有被写进仓库

上面第零节那些——两个产品面、两种形态、职责三不、最小闭环原理——**在这次讨论里确立了,但仓库里没有一处记录**。

最接近的两条(`three-layers` / `gmr-not-entailment`)锚在已删除的文档上(见问题四)。

**后果:** 下一个 agent(或三个月后的人)会从零重推,并大概率重复"GMR 是记忆工具"那个短路。这份文档存在,一半是为了这个。

---

## 八 · 这次实施暴露的问题

这一节是过程问题,跟产品问题分开。

### 这个仓库有两类东西,只有一类有 undo(代码可查)

```
可逆    源码 · 文档 · memories/*.md · tools/gate.py       git,一条命令
不可逆  .anchor/state/memory.db                            没有
        journal · bindings · links · sealed                schema.rs 逐表 RAISE(ABORT,'append_only')
```

`schema.rs` 的小标题就写着 *"Append-only — by trigger, not by good intentions"*。

**我把日志当源码用了。** 实施期间每一次 `gmr sync` 都当成"验证步骤"随手跑,而每一次都是往不可逆的那一半里写。

### 三个具体的过程失误

1. **动手前只读了接口,没读不变量。** 读了 `LinkStore` 的两个方法就得出"没有 unlink,加一个 relink",**没读 `schema.rs`**。整个方案建立在一个从没查过的事实上。
2. **验证看输出不看退出码。** `gmr sync >/dev/null 2>&1` 然后数边——检查的是希望看到的症状,不是那条命令有没有跑成功。两次静默失败的 sync 被当成"幂等通过"。
3. **用不可逆的操作清理可逆的错误。** 381 条链接是坏的但**有界**;为清掉它们做的 export/import 手术是**无界**的,它重建了整个 store,而我不知道我没恢复的表有谁在读。**清理比烂摊子本身危险一个量级。**

### 留下的残留(实测)

- `.anchor/state/memory.db` 里有 **381 条 `kind='cites'` 的边**。经 `carry_linked`,投递从 617 条涨到 2328 条(**3.8×**),涉及 659 个锚里的 531 个。append-only,删不掉。
- 孤儿锚 `crates/gmr-runtime/src/read.rs#Blind`——为一条已删除的笔记开的,`doctor` 报 `undeclared`。

### 顺带查出、不是本次造成的两件

- **`check` / `observe` 无 key 时走 `rt.anchors()`,包含 closed 锚**,而 closed 锚不入队 → 租不到就报"被别人占着"。今天能跑只因为旧库留着 closed 锚的队列残留行。同族的 `status` 明确过滤 closed。
- **`Blind::of` 把 `TimedOut` 映射为 `NeverAsked`** 抢在 `ReasonClass` 分支之前。**不是 bug**:传输层用同一个码表示"探针跑了但超时"和"预算在调用前就耗尽",两者不可区分,`NeverAsked` 是 claim 更少的那一半。

---

## 九 · 这次调查自身的缺陷

不写下来,下面的结论会被当成比实际更硬。

1. **"裸"组不是裸的。** CLAUDE.md 自动进每个 agent 的上下文。实测的其实是「CLAUDE.md」vs「CLAUDE.md + 坐标本地记忆」。
2. **B2 判据不硬。** "设计语义检索"没有无歧义的错误答案,两臂都把"已绑定=已复核"和"召回=猜测"分开了,那是站得住的答案。**B2 证明不了任何事。**
3. **B2 提示词预载了框架。** 我写的是 "GMR stores maintained memories… there is no way to ask which memories are relevant"——**我自己把"记忆系统"的框架塞了进去**,然后测它们会不会偏向记忆系统。
4. **语料对搜索是可发现的。** `memories/three-layers.md` 是仓库里的文件,agent 能 grep 到。被删的是它锚定的 `docs/GMR.md`,不是它自己。所以"锚定拓扑够不着"和"搜索找不到"是两件事,这次只区分开了一半。
5. **窄化的第一次测量是循环论证。** 我按符号名找片段,片段当然含符号名。重做后才得到 100% / 0% 那组数。

---

## 十 · 还没有答案的

- **宪法层能压到多小而不失去打断闭环的能力。** 那 4 条重心记忆 12.6KB,挂在每次编辑上太贵。这是信息论问题,而且可实验:注入 1KB 蒸馏版 vs 12.6KB 原文,比较打断率。
- **服务侧动态发现的事实源怎么锚。** 过敏原例子:AI 在分析配料时才发现要查"花生油是否致敏"。预先枚举 = 退回传统系统;运行时开长期锚 = 锚随对话无限增长;一次性读数 = 今天不存在。
- **注入在长会话中是否随上下文增长而失效。** 没测过。
- **最小信息量是不是可计算。** 从坐标出发求"使闭环正确的最小记忆集"——目标函数("闭环正确")依赖任务,选集合时任务未知,不可先验计算。图里有一个可计算的层级信号(锚定目标的类型),但那只是近似。

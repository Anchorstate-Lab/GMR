---
about: batteries/survey/src/matching.rs#report
watch: [sig, logic]
---

# 报告的字段集合是契约，两个分支必须一样

`report()` 吐的东西分两层，分界线是**它是关于谁的**：

- `REPORT` 里那九个是**报告级** —— 关于这次挑选本身，不属于任何一个候选
- `PER_CANDIDATE`（`at` · `facts`）是**被选中那一个**的坐标和事实，规则读作 `obs.at.x` / `obs.facts.x`

`REPORT` 只有这一份。`domains/coding/cli/src/contract.rs` 从这里 `pub use` 出去，
`unmet()` 拿它判「规则读的字段探针吐不吐」。**曾经是两份手写清单**，一份在这里、
一份抄在 contract.rs 里，而这里的注释亲笔写着 "Not enforced"。删掉第二份比检查两份强 ——
没有两份就没有分家。

## 两个分支的键集必须相等

`found: false` 那条早退路径和正常路径吐的键**必须一模一样**，
`both_branches_report_the_same_keys_and_they_are_the_declared_ones` 盯着这条。

理由不是整洁：一个只有某个分支才吐的键，对读它的规则来说是 `Absent` —— 而
「哪个分支跑了」恰恰就是那条规则在问的事。`roll` 和 `priority` 曾经只在
`found: true` 里出现，没炸纯粹是因为规则顺序把 `obs.exact == false` 排在所有
`Since` 规则前面，`obs.roll` 永远没机会被求值。**那是靠顺序侥幸正确，不是靠契约。**

## 优先级顺序不是实现细节

候选按命中向量做字典序比大小，所以**坐标项的顺序就是优先级**。
`[name, file]` 之下，只命中 `name` 的候选压过只命中 `file` 的。
探针作者写 `ITEMS` 的顺序，声明的是「哪个字段最能保住身份」。
`priority` 把这个顺序报出来，而不是藏在参数里。

`nth` 越界是错误，不是夹取。悄悄换一个候选，等于让锚去盯另一个东西而没人知道。

## matches 为什么没了

原来还有一个 `matches`，装全部并列候选的 `{at, facts}`。它的身份职责被
[[survey-roll]] 接管之后就只剩体积：本仓库 `layer::gmr-core` 一次转换的
facts 共 35,430 字节，其中 34,892 字节是它 —— 98%，而没有任何判据读它。
剩下那部分（其他并列成员的函数体哈希）对锚本来也没有意义：锚盯的是**一个**东西。

`MAX_BYTES` 那个上限当初就是为拦它设的。它走了以后同样宽的坐标只产出二十分之一，
上限还留着，但守的已经是 `roll` 了。

一处代价：`extract` 里那个「声明的 `at` 键必须从真实运行里回得来」的测试，
原来靠 `matches` 一次拿到全部候选，现在按 `nth` 逐个跑。**测试付这个成本，
生产不付** —— 这正是该有的分法。

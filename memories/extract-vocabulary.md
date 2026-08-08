---
about:
  - domains/coding/extract/src/lib.rs#Vocabulary
  - domains/coding/extract/src/lib.rs#every_key_a_probe_declares_comes_back_from_a_real_run
watch: [sig, logic]
---

# `at` 有两层，只有一层进语义闭包

`ITEMS` 里的键是**可匹配的**：它决定一个坐标挑中哪个候选，所以它住在语义闭包里，
改它就该换探针版本。`at` 里而不在 `ITEMS` 里的键（`form` · `surface` · `after`）
只是**可观测**，仅此而已。

两层不能重叠。**一个既参与挑选、又被当成轴去读的键，永远不可能动** ——
被挑中的候选按定义在它上面是相等的。`file` 那一维就是这么死的。

`every_matchable_key_is_one_the_probe_declares` 守 `ITEMS ⊆ at`：
声明一个探针根本不吐的可匹配键，会让每个写了它的 position 静默地匹配不上。

## 为什么 `Vocabulary` 故意在闭包外面

它约束的是**哪些 shape 喂得动这个探针**，不是探针**推导出什么**。
换一个 `handles` 扩展名不改变任何一次观测的结果，不该让全仓的 fact_address 翻篇。

代价是它跟候选表能各走各的：`Vocabulary` 写在这个文件里，候选是在闭包里面造的。
分家的时候，某个 shape 会去读一个没有候选携带的 `obs.at.<键>` —— 规则 fault，
或者更糟，那一维**永远不会动而没有人发现**。

所以每个声明的键都得从一次真实运行里回来。那个测试按 `nth` 逐个把并列候选跑一遍
再取并集 —— 原来它靠报告里的 `matches` 一次拿全，那个字段为了 98% 的体积被删了
（见 [[survey-report]]）。**测试付这个成本，生产不付。**

---
about:
  - domains/coding/cli/src/shapes.rs#seen
  - domains/coding/cli/src/shapes.rs#state_carries_exactly_what_some_guard_compares_and_nothing_else
watch: [sig, logic]
---

# state 里只能住参与判据的字段

顶层永远是这五个：`position` · `baseline` · `now` · `v` · `status`。
`baseline` 和 `now` 的字段集合恒等于全部 `Since` 维的 `field`，
`v` 的键集合恒等于全部维名。多一个都不行。

理由在基底那边：`should_still` 比的是**整个 State 相等**
（`crates/gmr-core/src/journal.rs`）。所以一个不参与任何守卫比较的字段，
照样让两次读数的 state 不同 —— 于是章节每挪一行就写一条 `Transitioned`，
而**没有任何一位亮**。`gmr edges` 会被灌满不是转换的转换，`gmr check` 却干净，
因为交付看的是位向量。

这不是假想。`facts.line` 差一点就这么进来：想让 `gmr status` 能显示章节在第几行，
顺手把它塞进 `reading()` 但不给它一维。`body_lines` 同理，也砍了。

**位置和体积这类只给人看的事实没有丢**：它们随观测进日志，`facts` 每次都在那儿。
要给人看就从观测里取，不要住进状态。渲染的缺口用渲染补，不用状态补。

一个推论：想加一维，就得同时想清楚它比什么。`Now` 维（像 `missing`）不写
`baseline`/`now`，所以它不进这两个集合 —— 它说的是「这份读数不是关于我的目标的」，
判据见 [[shapes-Dim]]。

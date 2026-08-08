---
about: domains/coding/cli/src/verbs/mod.rs#recapture
---

# 重钉基线必须重新看一眼，不能钉上一次的读数

把状态清回只剩 `position`、让 shape 自己的捕获规则重跑 —— 这个动作原来有三份实现，
`rebase` 一份、`accept` 的 table 分支一份、`accept` 的 vector 分支一份。前两份是抄的，
第三份是发明的，也只有第三份错：它钉 `state.now`，而那是**上一次 δ 写进去的**。

后果是实测的：改坏 → `check`（红）→ 把改动撤回 → `accept` → `check` ⇒ **还是红**。
过期读数成了新基线，好代码反倒"变了"，要 accept 两次才回得去。

正确做法本来就写在 [[delivery-standing]] 里，只是当时只应用到了 table shape：

> 把状态清回只剩 position，让 shape 自己的捕获规则重跑一遍。捕获不回去它就照实说
> `absent`，accept 不会把问题盖掉。

对 vector shape 一样成立 —— R1 `not exists(state.baseline) and obs.exact` 就是捕获规则，
目标不在了走 R2 报 `absent`，而不是钉一个谎。

**修法不是"加一次 observe"，是三份合成一份。** 重复实现本身就是这个 bug 的成因：
两份抄对了、一份没有，而没有任何东西会发现。

## 变了要问什么

`observe` 被从这里拿掉 → 立刻回到"钉上一次读数"。问：那份读数是哪次观测写的？

有第四处开始自己拼 `Restate { state: {position} }` → 它就是第四份实现。
问它为什么不能调这里。

顺序不能反：先 `revise` 再 `observe`。反过来会先观测一遍旧基线，日志里多出一次
毫无意义的转换。这两条仍是**两条独立日志条目** —— δ 的输入没有变（第 7 条），
观测归观测，判据修改归判据修改。

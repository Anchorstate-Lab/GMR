---
about: crates/gmr-core/src/journal.rs#fold
---

# closed 只累积，不重读

`closed` 一条一条累积，永远不清零。终结是**日志里发生过的一件事**，
不是对最终状态的重新解读。

差别在这里：如果每次都拿最终 state 去比 terminal 集合，那么「进过终结态、
后来被 Restate 挪出来」就会读成「没结束过」—— 历史被抹掉了。累积则不会。

`s.closed = s.closed || s.anchor.is_terminal(&s.state)` 那个 `||` 就是这条。

## 变了要问什么

`closed` 变成可以从 true 回到 false → 直接违反第 8 条。任何「重新计算 closed」
的重构都要先回答：进过终结态这件事，还看得见吗？

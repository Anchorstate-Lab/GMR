---
about: crates/gmr-core/src/anchor.rs#is_terminal
watch: [sig, logic]
---

# 基底比对 status，不解读它

第 4 条：没有固定状态词表，status 字符串由域定义，基底只拿它做 terminal 比对。
`is_terminal` 就是那句话的全部实现 —— 拿 `state.status()` 去 `terminal` 集合里查，
不匹配前缀、不认识大小写、不理解语义。

测试用 `"расчёт"` 当终结态不是玩笑，是判据：**只要基底开始"读"status，
就等于偷偷立了一份词表**，而那份词表永远只覆盖写它的人当时想到的那几个词。

第 8 条靠这个函数机械兑现：进了终结集合就 `closed`，不可逆（见 [[journal-fold]]）。

## 变了要问什么

这里出现任何字符串处理 —— trim、lowercase、`starts_with`、分隔符 —— 都是在解读。
问：域为什么不能自己把 status 写成它想要的样子？

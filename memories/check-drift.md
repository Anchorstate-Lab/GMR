---
about: domains/coding/cli/src/verbs/check.rs#drifted
watch: [sig, logic]
---

# 判据漂着的时候，check 上面说的话都不作数

`shapes::of()` 要求 `Transitions` 完全相等才认得出一个 shape。所以**任何一次
shape 改动之后、`accept --criteria` 之前**，全仓这个 shape 的锚都认不出来 ——
`delivers` 收到 `None`，直接退回边沿触发，笔记的 `watch:` 整个失效。

`gmr status` 一直会报这件事。`gmr check` 以前一个字不说，而 check 才是天天跑的那个。

这跟 `Body::Table`/`Vector` 双轨是同一类病：**一个判据在某种状态下静默停止生效**。
双轨拆掉之后，二义性没死，只是从枚举挪进了 `Option` —— `None` 同时意味着
「这个锚用手写规则」（该退回边沿触发）和「这个锚的判据漂了」（该报出来）。
check 报出漂移，`None` 在这条路上就只剩前一个意思了。

所以它印在最后，而且明说上面的结论不可信。放在前面会被后面那句
「n of N handed a memory back」盖过去，而那句在漂移期间恰恰是错的。

**这不进 `gate.sh`。** 它要读 `.anchor/state/memory.db`，而 gate.sh 六条从没漂过，
一半原因就是它不碰任何锚 —— 让 CI 因为某人本地没跑 `accept` 而红，就是把
「要人看的信号」变成了构建失败。gate.sh 查源码树的恒等式；这一条查的是
某一个仓库的锚存储对不对得上它自己的笔记，是另一个对象。

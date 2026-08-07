---
about: crates/gmr-core/src/probe.rs#ProbeRef
---

# 锚写下的，不是真正跑的

`ProbeRef` 是**锚上写的那个名字**，`Derivation` 是**这台机器上真正跑的那个东西**。
两者不是一回事，而且必须留着差。

一个新克隆里，declaration 原样旅行过来了，制品没有 —— 这时 `ProbeRef` 存在、
`Derivation` 解不出来。这正是 doctor 的 stranded 要报的那件事，也正是
「问 Artifacts 而不是问 observer」会 100% 误报的原因：它拿声明去问了一个
只认识 shell 制品的仓库。

## 变了要问什么

任何把这两个合起来、或者用其中一个去推另一个的代码 → 它假设了「写下的就是跑的」。
在新克隆和换机器这两个场景下这个假设是假的。

## `ProbeName` 是名字，不是哈希

它必须**扛过一次引擎升级不变**。声明写的是名字，这台机器解析出什么是 derivation
的事（见 [[probe-Derivation]]）。名字长得像版本会被当场拒绝，
判据在 [[probe-check_probe_name]]。

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

## `name` 这个槽位曾经叫 `artifact`

字段上那行 `#[serde(alias = "artifact")]` 是这句话的全部实现。这个槽位在只装版本的
年代叫 `artifact`，当时写下的条目仍然说着 `artifact`，而且要按原样读回来 ——
日志只增不改，改名不能让旧条目失效。

**写路检查，读路不检查。** `check_probe_name` 会当场拒掉 64 位 hex，而那些旧条目的
名字位上装的正是 64 位 hex。两者不打架：校验器只挂在 `try_new` 上，
而 `string_newtype!` 生成的是 `#[serde(transparent)]` 的 Deserialize，反序列化
一个字都不验。**新名字进不来，旧条目读得回去** —— 这是同一个 newtype 上两条路的
分工，不是漏了一处校验。

## 删掉那行 alias，本仓库不会红

这个仓库的日志里一条说 `artifact` 的条目都没有（存储在改名之后重建过）。
所以删掉它 `cargo test` 全绿、`gate.sh` 全绿，代价落在别处的旧日志上：
`name` 没有 `default`，缺了这个键整条 `Entry` 反序列化失败，那个锚从此读不出来 ——
不是报错说少了个字段，是整段历史打不开。

**唯一会开口的是这个锚。** 删掉 alias 报的是 `ProbeRef` 的 `signature-changed`
（属性算签名，见 [[ast-signature]]），递回来的就是这一篇。这条约束一度写在
[[probe-Derivation]] 里 —— 那篇锚在 `Derivation` 上，而 `Derivation` 不会因为
这行改动动一下。**记忆挂错了锚，等于没挂。**

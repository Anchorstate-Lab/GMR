---
about:
  - domains/coding/cli/src/verbs/check.rs#drifted
  - domains/coding/cli/src/verbs/mod.rs#swapped
watch: [sig, logic]
---

# check 得说出自己什么时候不作数

有两种情况会让 check 上面那些结论失效，成因不同、补救也不同，所以是两段报告：
`drifted` 说**判据**不作数，`swapped` 说**读数**不可比。两个都印在最后，
各带各的补救动词。

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

# 换了仪器的读数，跟基线不可比

`swapped` 比的是**这个锚站着的那次读数是谁取的**（`view.derivation`）和
**这个 build 现在解析出什么**（`rt.instrument`）。不等，就说明基线是另一台仪器量的。

不报的后果实测过：往 `batteries/survey/src/matching.rs` 加一个常量再重编，
四个抽取器全换版本（ast-map `1e1ac5ee`→`48db5084`，其余三个同理），
全仓 56 个锚的基线一次性变得不可比 —— 而 `check` 干干净净退出 0，
`status` · `doctor` · `health` 也都一个字不说。当时唯一知情的是 `rebase --all`
自己的选择器，**那是个选择器，不是报告**：它只在你已经决定要 rebase 的时候才开口。

这一维两头都可能骗人：输出没变时全仓静默（版本动了行为没动），输出变了时
每个锚都报 `signature-changed`（看起来像有人改了代码）。所以这段话不说「变了」，
说的是**这一轮分不出是哪一种**。

## 为什么算在 check 里，而不是让 rebase 自己喊

`rebase` 要 `--why` 并且封存理由，它是**动手**的动词。「站在不可比的基线上」这件事
得在人动手之前就知道，而天天跑的是 `check`。这跟上一节同一条：
判据/读数失效是**要人看的信号**，不是构建失败，所以它进 check 不进 `gate.sh`。

一份实现，两个调用方 —— `swapped` 住在 `verbs/mod.rs` 而不是 check 里，
因为 `rebase --all` 挑的就是同一批锚。理由见 [[verbs-recapture]]：
三份抄成两对一错，是那个 bug 的成因，不是它的表现。

## 还没修的那一半：这个事实会被观测吃掉

`swapped` 是从**最新那次观测**的 derivation 推出来的，所以谁先观测谁就把它抹掉。
check 之所以还能报，只是因为它在自己的 observe 循环**之前**算这一段
（挨着 `drifted` 那一行）—— 顺序反过来就永久静默了。

**但 `pass` 不会报。** 部署里 `pass` 按节奏跑，它一观测就把新 derivation 写进去，
人再跑 `check` 就什么也看不见了。这正是 [[shapes-Dim]] 给 `Since` 维写下的那条：
「变过了」是过去式，谁先观测谁就消费掉。

真正的修法是把这件事记在消费不掉的地方 —— 一位，或者一条日志条目 ——
而不是从最新读数现推。那是**基底的判据变更**，得 owner 拍板（第 5、第 7 条），
所以这一版只补了报告，没补留存。在那之前：**重编之后先跑 `check`，再让 `pass` 上。**

**这不进 `gate.sh`。** 它要读 `.anchor/state/memory.db`，而 gate.sh 六条从没漂过，
一半原因就是它不碰任何锚 —— 让 CI 因为某人本地没跑 `accept` 而红，就是把
「要人看的信号」变成了构建失败。gate.sh 查源码树的恒等式；这一条查的是
某一个仓库的锚存储对不对得上它自己的笔记，是另一个对象。

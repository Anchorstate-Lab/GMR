---
about:
  - domains/coding/cli/src/delivery.rs#delivers
watch: [sig, logic]
---

# 交付问的是「还有没有没处理的」，不是「这次动了吗」

`check` 曾经只认 `Observed::Transitioned`。第二次观测状态没动就是 `Still`，
不报、退出码 0 —— **累积位只喂给了 `status`，没喂给交付**。
第 1 条决定「一旦置 1 保持到人重新确认」在显示层兑现了，交付层没有。

后果是实测的：本仓库的 `doctrine::red-cards` 坏着（盯的章节不存在），
`doctor` 印 `section-gonex1`，而 `check` exit 0 —— **CI 是绿的**。
改签名之后连跑两次 check，第二次也是 "nothing moved"，而 `status` 里 `v.sig` 还挂着。

现在两条路，**按这个锚有没有 shape 分，不是按 state 长什么样猜**：

| | 判据 | 谁定 |
|---|---|---|
| 有 shape | 订阅的位里有没有 1 | 位是累积的，`accept` 清 |
| 手写 rules（`of()` 返回 `None`） | 退回边沿：这次转换了才递 | —— |

第二条必须留着。手写规则表没人声明过什么算安定，对它做电平就等于「永远不绿」。
退回边沿是**已知的降级**，不是遗漏。

曾经有第三条 —— table shape 靠一张手写的 `settled` 白名单。那是双轨的产物：
同一个问题（这个状态还要不要人动手）有两套答案，而白名单那套没有订阅。
所有内置 shape 向量化之后它消失了：安定就是**全部位落下**，推出来的，不是列出来的。

**问的是声明，不是数据。** `delivers` 收一个 `Option<&Shape>`，不再拿
`state` 里有没有 `v` 去反推 shape 的种类 —— 那是结构性类型判断，而且手写规则的锚
和 table shape 在那种判断下不可区分。

## 变了要问什么

`settled` 里加了新 status → 问：**人看完这个状态之后，还需要做什么吗？** 
不需要 → 它是安定的。需要 → 它不是，哪怕它听起来很正常。
`captured` 是安定的；`added` `count-moved` `section-gone` `absent` `drifted` 都不是。

第三条路被删掉（比如「不认识就当安定」）→ 手写 rules 的锚会永久静默。
反过来「不认识就当未安定」→ 它们永远 exit 1。两个都是错的，所以这里必须是三分支。

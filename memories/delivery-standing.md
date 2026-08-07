---
about:
  - domains/coding/cli/src/delivery.rs#delivers
  - domains/coding/cli/src/shapes.rs#settled_of
watch: [sig, logic]
---

# 交付问的是「还有没有没处理的」，不是「这次动了吗」

`check` 曾经只认 `Observed::Transitioned`。第二次观测状态没动就是 `Still`，
不报、退出码 0 —— **累积位只喂给了 `status`，没喂给交付**。
第 1 条决定「一旦置 1 保持到人重新确认」在显示层兑现了，交付层没有。

后果是实测的：本仓库的 `doctrine::red-cards` 坏着（盯的章节不存在），
`doctor` 印 `section-gonex1`，而 `check` exit 0 —— **CI 是绿的**。
改签名之后连跑两次 check，第二次也是 "nothing moved"，而 `status` 里 `v.sig` 还挂着。

现在三条路，按 state 的形状分：

| state | 判据 | 谁定 |
|---|---|---|
| 有 `v` | 订阅的位里有没有 1 | 位是累积的，`accept` 清 |
| 无 `v` | `status` 在不在 shape 的 `settled` 里 | `settled_of` |
| shape 不认识（手写 rules） | 退回边沿：这次转换了才递 | —— |

第三条必须留着。手写规则表的 shape 没人声明过什么算安定，
对它做电平就等于「永远不绿」。退回边沿是**已知的降级**，不是遗漏。

`settled_of` 是唯一的声明处，`check` 和 `accept` 都读它 —— 两个动词共用一份判据，
不会各说各话。table shape 的 `accept` 也因此有了意义：把状态清回只剩 position，
让 shape 自己的捕获规则重跑一遍。捕获不回去（比如章节真的没了）它就照实说 `absent`，
accept 不会把问题盖掉。

## 变了要问什么

`settled` 里加了新 status → 问：**人看完这个状态之后，还需要做什么吗？** 
不需要 → 它是安定的。需要 → 它不是，哪怕它听起来很正常。
`captured` 是安定的；`added` `count-moved` `section-gone` `absent` `drifted` 都不是。

第三条路被删掉（比如「不认识就当安定」）→ 手写 rules 的锚会永久静默。
反过来「不认识就当未安定」→ 它们永远 exit 1。两个都是错的，所以这里必须是三分支。

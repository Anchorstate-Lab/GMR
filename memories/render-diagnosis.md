---
about:
  - domains/coding/cli/src/render.rs#diagnosis
  - crates/gmr-runtime/src/read.rs#AnchorView
watch: [sig, logic]
---

# 「为什么 missing」的答案一直躺在日志里，只是没人渲染

位向量说的是**哪一维动了**，说不出**为什么**。而 `missing` 亮起来的时候，
人真正要问的是「文件没了，还是名字没了」。

那个答案在探针的报告里，每次观测都在，只是从来没被打印出来：

```
found      = true          文件在
exact      = false         但不是精确命中
matched    = ["file"]
missed     = ["heading"]
candidates = 7             那文件里有 7 个并列的 heading
```

`doctrine::red-cards` 就是这样躺了一整段历史。`gmr check` 打的是
`doctrine::red-cards   absent` 一行，人拿着这行什么也做不了；现在多打一行
「file matched, heading did not — this reading is about whichever of 7 others was closest」，
问题当场就清楚了。

## 为什么走 `AnchorView.facts` 而不是 `Observed`

`Observed` 是「这一次观测发生了什么」，`AnchorView` 是「这个锚现在什么样」。
诊断要回答的是后者 —— 打开 `gmr status` 的人没有刚跑过一次观测。
`read()` 早就从 `latest` 里取过 `sighting` 和 `derivation` 了，`facts` 是同一个对象上
第三件不需要解释就能给出去的东西。

**基底不解释它**。`Facts` 原样传出来，怎么念是域的事 —— `diagnosis` 认
`gmr.probe-coord.v1` 这个 schema，别的探针（脚本探针）它一句话都不说，
返回 `None`。这跟第 3、11 条是一回事：基底能取字段，不解释字段含义。

## 为什么不是把这些塞进 state

试过这个念头，被 [[state-schema]] 挡回来了：state 里多一个不参与判据的字段，
`should_still` 就会把每次读数都判成不同，写一条没有任何位亮的转换。
**渲染的缺口用渲染补。** 观测已经进日志了，取出来念给人听不需要状态帮忙。

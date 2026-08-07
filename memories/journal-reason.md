---
anchors:
  - key: crates/gmr-core/src/journal.rs#reason
    probe: ast-map
    position: { file: crates/gmr-core/src/journal.rs, name: reason, kind: function }
    shape: contract
---

# 派生，不是并排存

`FailureCode::reason()` 把 code 映射到基底真正会去动作的那个类。它是**算出来的**，
不是跟 code 并排存一份 —— 存两份，早晚有一天某个新 code 忘了更新映射，
两边开始各说各话，而且没有任何东西会发现。

这跟 `AnchorState.closed` 是同一条纪律：能从已有事实推出来的，就不要再存一份。

## 变了要问什么

出现了「某个 code 的 reason 要覆盖」的需求 → 说明 code 的划分错了，
该拆 code，不该给映射开后门。

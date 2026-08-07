---
anchors:
  - key: crates/gmr-core/src/journal.rs#scan
    probe: ast-map
    position: { file: crates/gmr-core/src/journal.rs, name: scan, kind: function }
    shape: contract
---

# 只有一份投影

`scan` 走一遍日志，每条之后把当时的 fold 交出去。`fold` 就是它的最后一格。

需要知道「沿途发生了什么」的消费者到这里来，**不要再写第二份投影**。
两份投影迟早会漂开，而且不会有任何东西发现 —— 因为两边各自都是自洽的，
只有拿同一份日志同时喂给两边才看得出来，而没人会那么做。

这条是第一性的：当前状态只能来自日志投影，那么投影就只能有一份。

## 变了要问什么

出现了第二个遍历 `entries` 并重建状态的函数 → 不管它叫什么，它就是第二份投影。
问它为什么不能是 `scan` 的一个 callback。

---
anchors:
  - modules::gmr-core
  - surface::gmr-core
---

# gmr-core — 词汇

GMR 的名词。**不知道怎么拿到事实、怎么求值、怎么存。**

## 公开面变了要问什么

新增的东西如果带上了「怎么拿 / 怎么算 / 怎么存」中的任何一件，它就不再是词汇表了 ——
那三件事分别住在 `gmr-probe` 的契约、`gmr-expr` 的求值、`gmr-store` 的接口里。

零 workspace 依赖是纯根的定义，`gate.sh` 守着。

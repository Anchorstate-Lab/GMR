---
about: crates/gmr-core/src/journal.rs#Versions
---

# 一次观测背后的三重身份，永远不许合并

`declaration`（锚上写的那句话）、`derivation`（真正导出这些事实的东西）、
`evaluator`（当时的求值器）—— 三个各自独立演化、各自以不同方式失败。
合并任意两个，就是在对第三个撒谎。

具体地：探针脚本改了但锚没动 → 只有 derivation 变；锚换了探针名 → 只有
declaration 变；求值器升级 → 只有 evaluator 变。任何一次「状态动了」的解读，
都要先排掉这三个里哪个动了。合并之后就排不掉了。

## 变了要问什么

有人想把三个压成一个「version」→ 问他：探针改了和锚改了，你打算怎么区分？
Phase B 的多探针会把 declaration 和 derivation 下沉到每份读数，evaluator
留在观测级 —— 那是拆得更细，不是合并。

## 三个字段各是什么

```
declaration   锚上写的那句话
derivation    真正导出这些事实的东西，以及那个身份可不可证
evaluator     当时在跑的求值器
```

第二个自己还带着 `Verifiability`（见 [[probe-Verifiability]]）——
「导出它的东西是什么」和「那个说法可不可信」也是两件事，也没有合并。

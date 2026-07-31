---
anchors:
  - doctrine::decisions
  - doctrine::red-cards
  - dead::Baseline
  - dead::BaselineEntry
  - dead::Delta
  - dead::Expect
  - dead::ExpectValue
  - dead::FacetName
  - dead::FacetOutcome
  - dead::FacetValue
  - dead::Facets
  - dead::FactStore
  - dead::SourceArchive
  - dead::delta_of
  - dead::located_at
  - dead::prev
  - dead::streak
---

# 判据本体

`CLAUDE.md` 是立场、红牌与死概念的唯一出处。这些锚盯的是它本身，以及它宣布死掉的名字
有没有回到代码里。

## 十三条或红牌的指纹变了要问什么

指纹变了只有两种可能：**owner 改了判据**，或者**有人改了不该改的**。
两者在观测上长得一模一样，基底分不出来 —— 所以它把这一节交回给你，由你说是哪一种。

上一轮腐坏的机制正是第三环：AI 写论证 → 推翻 owner 的决定 → 论证进文档 →
下一轮读到文档把论证当判据。这两个锚盯的就是那一环。

## dead:: 报了要问什么

每个锚盯一个死概念在 `crates/` 底下的出现次数。**基线不是零** ——
`located_at` `prev` `streak` 各有 1 处，在 `crates/gmr-expr/src/parse.rs` 那条
断言「这些槽会被拒绝」的负测试里。只有增加才是信号。

它锚的是**契约的影子**（文本里有没有这个串），不是契约。影子会在契约没动时动，
也会在契约动了时不动。用它判断「这个词本该绝迹」是成立的；用它判断别的不成立。

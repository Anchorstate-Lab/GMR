---
anchors:
  - key: doctrine::decisions
    probe: prose-map
    position: { file: CLAUDE.md, heading: "一、这十三条是 owner 定的，不要重新论证" }
    shape: fingerprint
  - key: doctrine::red-cards
    probe: prose-map
    position: { file: CLAUDE.md, heading: "四、红牌 —— 违反了不会有人发现的那些" }
    shape: fingerprint
---

# 判据本体

`CLAUDE.md` 是立场与红牌的唯一出处。这两个锚盯的是它本身。

## 指纹变了要问什么

指纹变了只有两种可能：**owner 改了判据**，或者**有人改了不该改的**。
两者在观测上长得一模一样，基底分不出来 —— 所以它把这一节交回给你，由你说是哪一种。

上一轮腐坏的机制正是第三环：AI 写论证 → 推翻 owner 的决定 → 论证进文档 →
下一轮读到文档把论证当判据。这两个锚盯的就是那一环。

这一层是**正交于代码颗粒度的**。`crates/` 底下的锚盯的是某段代码有没有动；
这两个锚盯的是「判断那段代码该不该动」的依据有没有动。前者变了要看代码，
后者变了要重读全部记忆。

## red-cards 报 section-gone 要问什么

「四、红牌」这一节在 `5f6b22d rewrite claude.md` 里就没了，而这个锚一直报
`captured` —— 因为 `file` 命中、`heading` 没命中，探针退回文件里第一个 heading，
而当时的捕获规则不看 `exact`，把那个错的章节钉成了基线。两个 doctrine 锚
因此指纹完全相同、都指向第 7 行。

规则顺序修好之后它才开始说实话。它现在报 `section-gone` 是**对的**，要问的是：
红牌那一节是被有意删掉的，还是搬走了？前者就关掉这个锚，后者就把 heading 改对。
在你回答之前它会一直报，这正是它该做的。

---
about:
  - domains/coding/cli/src/shapes.rs#vocabulary
  - domains/coding/cli/src/memories.rs#tombstones
watch: [sig, logic]
---

# 删掉一个词的那个 commit，同时给它立碑

锚只在**坐标动了**的时候把记忆递到人面前。而一篇记忆点着一个已经不存在的
status、shape 或字段，坐标一动没动 —— `gmr check` 干干净净，那句话就那么错着。

实测过一次：`e11bc73` 在同一个 commit 里删掉 `Body::Table`、重写了
`delivery-standing.md` 的交付三条路，却没注意到同一段下面「`captured` 是安定的；
`added` `count-moved` `section-gone` 都不是」整句已经指着不存在的东西。
一篇里四个。全仓七个，散在四篇。

## 为什么是墓碑名单，不是「必须存在」的断言

反过来写 —— 扫记忆里的反引号，凡不在当前词表里的就报 —— 零误报做不到。
本仓库的记忆里有一百多个反引号 token：`file` `name` `logic` 是活的轴，
`accept` `rebase` 是动词，`pub` `const` `use` 是 Rust。**散文里认不出哪个是词表引用。**

墓碑名单反过来：只列**确实被删掉的词**，零误报。代价是删词那天要加一行 ——
而那一天恰恰是窗口开着的时候，人正看着那个词消失。

## 为什么退出码是 0

`moved-file` 在 [[shapes-Dim]] 里是**正确的**：那一整段就是在记录这一维为什么被删。
一篇记忆本来就该点名它埋掉的东西。所以「提到退役词」不可判定 —— 报出来，
让人分哪些是墓碑、哪些是漏改。跟 `long-hand` 同级。

`nothing_is_both_retired_and_shipping` 守着名单不跟词表打架：一个词同时在两边，
会让一篇正确的记忆被报成过期。

---
about: domains/coding/extract/src/ast.rs#naming
watch: [sig, logic]
---

# `name` 只给能被单独指名的那些 kind

`路径#名字` 这个坐标，人的意思永远是「这个文件里叫这个名字的那**一个**东西」。
所以 `name` 只留给能被单独指名的候选 —— function · module · type。另外两类各有自己的键：

- `callee`：**提到**一个名字而不是引入它 —— `call` · `import`
- `member`：是某个类型的一部分，身份要带上属主 —— `field`。
  `reason` 这个字段的身份是 `Attempt::reason`，不是 `reason`

不这么切的后果是实测出来的。`crates/gmr-core/src/journal.rs#fold` 当时是 **8 个候选**，
锚到了一个调用点；`#reason` 锚到了同名的 struct 字段而不是 `fn reason`。
两个都报 `exact=true`，`contract` 的 missing 规则挡不住，`status` 一路显示正常。
`nth=0` 挑中谁取决于**遍历顺序** —— 也就是说锚在盯什么，取决于 tree-sitter 怎么走树。

**没有丢掉任何东西。** `{file, kind: "call"}` 照样列得出全部调用点，
`{file, member: "reason"}` 照样指得到那个字段。只是它们不再能跟定义打平手。

## 变了要问什么

这个 match 加了新分支 → 问的不是「它像不像定义」，而是这一句：
**能不能在文件里用一个名字单独指到它？** 能 → `name`；
只是提到别处的名字 → `callee`；要带属主才能指到 → `member`。

这个函数动了会换探针版本（`ast.rs` 在 `build.rs` 的语义闭包里），
全部 ast-map 锚会报 instrument swapped —— 那是对的，输出确实变了。
`Vocabulary.at` 不在闭包里，往词表里加键本身不换版本。

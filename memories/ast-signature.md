---
about:
  - domains/coding/extract/src/lang.rs#Table
  - domains/coding/extract/src/ast.rs#members
  - domains/coding/extract/src/ast.rs#attributes
watch: [sig, logic]
---

# 签名是这个定义对调用方的承诺，不是 tree-sitter 给了哪些字段

`shape_fields` 用 `child_by_field_name` 取，只能拿到语法里**有名字的字段**。
而「改了它每个调用方都要改」的东西有一半不在字段里：

| | 住在哪 | 拿它的机制 |
|---|---|---|
| 参数 · 返回类型 · 类型参数 | 字段 | `shape_fields` |
| `async` `unsafe` `const` · where 子句 | 无字段的子节点 | `shape_kinds` |
| `#[derive]` `#[deprecated]` · TS/Python 装饰器 | **前驱兄弟** | `Attrs::Before` |
| struct 字段 · enum 变体 · trait 方法签名 | 子节点的 body 列表 | `members` |

这四条不是四个特例，是同一句话在语法树里的四个落点。判据是**去掉它，调用方还能不能
原样编过**；能，它就不属于签名。

## 三个具体判断

**`Attrs::Before` 一个变体够了。** 实测三种语言都是前驱兄弟：Rust 的
`attribute_item`、TS 的 `decorator`（在 `export_statement` 里排在 `class_declaration`
前面）、Python 的 `decorator`（在 `decorated_definition` 里）。要用
`prev_named_sibling` 而不是 `prev_sibling` —— TS 的 `export` 是匿名 token，
夹在中间会把循环打断。

**`NOISE` 是黑名单不是白名单。** 白名单让新出现的属性**静默**，黑名单让它出声，
然后你决定要不要闭嘴。「系统不允许静默失败路径」只允许后者。名单里那九个
（`allow` `warn` `deny` `expect` `inline` `cold` `doc` `rustfmt` `clippy`）的共同点是
去掉它们调用方一个字都不用改。`serde` 不在里面 —— 它改的是线上格式。

**type 的 shape 是它的成员，body 只剩成员的实现。** struct 没有 `parameters` 也没有
`return_type`，所以它的 shape 曾经恒空，本仓库二十五个 contract 锚里有十个带着一条
死轴在跑，而「加了一个字段」报的是 `logic-changed` —— 把「看所有构造点」说成了
「重读实现」。拆开之后 struct 根本没有实现，加字段是签名变更；trait 的默认方法体
仍然独立驱动 `logic`。

## 变了要问什么

往 `shape_fields` 或 `shape_kinds` 加东西 → 问那条判据：去掉它调用方还能编过吗？
不能 → 它属于签名。能 → 它是噪声，加进去等于教人忽略这一维。

`NOISE` 加一项 → 说出「去掉这个属性，哪个调用方都不用动」的理由。说不出就别加。

`members` 的展开方式变了（比如开始递归进嵌套类型）→ 全部 type 锚的 sig 变，
而且是**判据变更不是事实变更**，走 `rebase --all`，别当成漂移接受。

这三个函数都在 `build.rs` 的语义闭包里，动一次就换一次探针版本、全仓 `rebase`。
所以要改就一次改完 —— 见 [[probe-Derivation]]。

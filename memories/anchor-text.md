---
about: crates/gmr-core/src/anchor.rs#text
watch: [sig, logic]
---

# 规则的身份是它的源文本，哈希成 JSON 字符串

`Expr::text` 把规则源码哈希成 `ContentHash`，哈希的对象是
`Value::String(source)` —— **不是裸字节**。这样规则的身份跟其余所有内容地址
走同一条规范化路径，一个规则表的哈希和一份 state 的哈希可比。

那句 `.expect(...)` 是站得住的：规范化只在结构太深或数字非有限时失败，
而字符串标量既不递归也不是数字。这是**由类型保证的不可能**，不是没处理的错误。

## 变了要问什么

哈希对象从 `Value::String` 换成别的（裸字节、加了盐、带上 hash 字段）→
全部锚的 `declaration` 哈希变，`sync` 会把每一个锚都报成判据漂移。

`content_hash_of` 变得可能对字符串失败 → 那句 expect 就成了 panic 路径，
必须改成 `Result`。见 [[addr-canonical_number_string]]。

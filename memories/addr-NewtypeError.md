---
about: crates/gmr-core/src/addr.rs#NewtypeError
---

# 校验失败要说出是哪个 newtype

`string_newtype!` 的校验器拒了一个值。带 `type_name` 是为了让**包装它的调用方**
能分辨是哪一个 newtype 出的错，而不用去解析 `reason` 那句人话。

这条是 15472be 那次提交换来的：原来 `try_new` 返回 `String`，调用方要么整串透传，
要么写字符串匹配。结构化错误的全部意义就是别让上层去 parse 下层的散文。

## 变了要问什么

`reason` 有没有开始被程序读？如果有代码在 match `reason` 的内容，说明这里缺一个
真正的枚举，字段该升格。

---
about: crates/gmr-core/src/addr.rs#write_array
---

# 深度计数必须在错误路径上也回退

`write_array` 和 `write_object` 用立即执行的闭包 `(|| { ... })()` 把主体包起来，
拿到 result 之后再 `self.depth -= 1`，最后才返回。这不是风格 —— 主体里每个 `?`
都可能提前返回，如果直接写 `self.depth += 1; ...?; self.depth -= 1;`，
一次失败的规范化就会永久抬高深度计数，之后所有调用都在错的基线上判断
`MAX_CANONICAL_DEPTH`。

同一个规范化器实例跨多次 `write` 复用时这条才致命。`canonical_write` 每次新建一个，
所以今天摸不到；改成复用就会。

## 变了要问什么

主体里新增的提前返回，有没有绕过 `depth -= 1`？把闭包换成 `?` 直接写、或者引入
`return`，都会重新打开这个洞。

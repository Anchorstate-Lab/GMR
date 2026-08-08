---
about:
  - domains/coding/extract/src/lib.rs#for_extension
  - domains/coding/extract/src/lib.rs#root_of
watch: [sig, logic]
---

# 扩展名路由和「看哪一段」，两件都不能从进程里拿

## `handles` 让 `about:` 路由，而 CLI 不认识任何语言名

`coord::route` 拿坐标的扩展名去问 `for_extension`，探针自己声明认哪些。
所以 CLI 里没有一处写着 "rust"、"typescript"。空的 `handles` 表示这个探针
不走这条路 —— 得在完整式里点名。

只有 ast-map 能吃 `路径#名字`：那个坐标形状产出 `{file, name}`，别的探针的词表
都不匹配。prose-map 要的是 `heading`，所以要么点名，要么 `wanted` 把 `name` 丢掉，
锚就静默地盯了整个文件。

## 要看的那一段来自 params，不来自进程

`root_of` 从 params 取 `root`，不从当前工作目录推。params 进声明哈希，
所以**锚说得出自己当初的意思**；而进程的 cwd 谁跑谁不一样，同一个锚在两台机器上
会观测两棵不同的树，日志对不上，而且没有任何地方记着这个差别。

`layer::*` 那六个锚就是靠 `params: {root: crates/X}` 把范围收到一个包上的
（见 [[layers]]）。

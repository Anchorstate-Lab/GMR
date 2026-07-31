---
anchors:
---

# 记忆的格式

一份记忆挂在若干个锚上。锚机械地观测，记忆写的是**观测约束、但决定不了的那件事**。

```
front-matter   anchors: 挂在哪几个锚上。这一栏必须是真的 ——
               `anchor bind <文件> --anchors <键>` 跑过才算数
正文           这些锚动了，该重新问什么
```

**不写代码里有的东西。** 模块名单、公开面、依赖清单都由锚的 `matches` 实时吐出来，
`anchor read <键>` 看得到。手册抄一份只会跟代码分家 —— 这份目录上一轮就是这么烂的：
十份手册在 front-matter 里点名了约四十个锚，一个都不存在。

**不写不变量清单。** 机械可查的约束是 `gate.sh` 里可执行的检查，不是文档里打勾的条目。

## 锚的键

```
modules::<包>    该包的 pub mod 名册
surface::<包>    该包的 pub fn 名册与签名
packages::       全仓库的 Cargo.toml 名册
dead::<名字>     一个死概念在 crates/ 底下的出现次数
doctrine::       CLAUDE.md 某一节的内容指纹
tests::roster    测试名册
```

坐标写在 `anchors.toml` 的 `position` 里，探针一律是 `batteries/probe-*` 的通用探针。

---
anchors:
---

# 记忆的格式

一份记忆挂在锚上。锚机械地观测，记忆写的是**观测约束、但决定不了的那件事**。

```
front-matter   about: <坐标>   一行式，探针/形状/位置由坐标推出来
               anchors: [...]  完整式，坐标有歧义时手写 position
               watch: [...]    只有这些维动了才把这份记忆递回来（可选）
正文           这些维动了，该重新问什么
```

## 颗粒度：一个锚一段代码，一份记忆代替那段代码的注释

坐标写成 `路径#名字`，指向文件内部一个具体的定义：

```
crates/gmr-core/src/addr.rs#write_array
crates/gmr-core/src/journal.rs#Versions
```

这一层用 `contract` 形状，向量是 `missing · sig · logic · file · line`，
默认订阅 `missing, sig, logic` —— 行号漂移不打扰人，签名和实现变了才打扰。
位是累积的：置 1 之后一直留着，直到 `gmr accept <坐标> --why "..."`。

**代码里不写这些记忆已经说过的话。** 注释和记忆各存一份，两份会分家，
而且没有任何东西会发现。要说的话说在这里，代码里只留必要的英文短注释。

## 正交的那一层

```
doctrine::   CLAUDE.md 某一节的内容指纹（fingerprint 形状）
```

`crates/` 底下的锚盯的是某段代码有没有动；`doctrine::` 盯的是「判断那段代码
该不该动」的依据有没有动。前者变了要看代码，后者变了要重读全部记忆。

## 两条纪律

**不写代码里有的东西。** 名单、公开面、依赖清单由锚实时吐出来，`gmr status` 看得到。
手册抄一份只会跟代码分家。

**不写不变量清单。** 机械可查的约束是 `gate.sh` 里可执行的检查，不是文档里打勾的条目。
记忆写的是**为什么**，不是**是什么**。

## 已知的坑

裸坐标 `文件#名字` 在细颗粒度下会撞上调用点和字段声明 —— `ast-map` 把
`call_expression`、`field_declaration` 也当成同级候选。`journal.rs#fold` 裸着写
会锚到一个调用点（8 个候选），`journal.rs#reason` 会锚到同名字段。
这三个坐标用 `anchors:` 完整式写死了 `kind: function`。

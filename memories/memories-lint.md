---
about: domains/coding/cli/src/memories.rs#superfluous
watch: [sig, logic]
---

# 逃生口该不该用，靠走一遍路由来判，不靠约定

笔记的正规形式是 `about: <坐标>`；完整式 `anchors: - key/probe/position/shape`
是逃生口。问题是「什么时候算真的需要逃生口」如果写成文档里的约定，
就只能靠人自觉 —— 而这正是 owner 说的「不应该靠个人去维护」。

所以判据是可执行的：**把 `key` 当坐标扔进 `coord::route`，看推出来的东西
跟手写的一不一样。** 一样，且没有 `rules` / `terminal` / 非默认 `params`，
那这份完整式什么也没多说，`about:` 一行就够。

这样「逃生口的四个理由」不是列表，是这个函数的四个分支，跟 README 里那四条一一对上：

```
① 手写了 rules 或 terminal          两个理由共用一条 early return
② 非默认 params                     early return
③ coord::route 直接 Err             早退——探针根本不吃这个坐标
④ 路由推出来了，但探针或 position 跟手写的不一样   末尾那个布尔式
```

前三条是 early return，第四条是函数末尾的返回值 —— **不是四个 early return**。
分支数跟理由数对得上是有意的：文档里那份列表是从这个函数抄下来的，不是反过来。

## 变了要问什么

`coord::route` 变得更能干（比如坐标语法支持 `kind` 或 `member`）→ 这个函数
自动会把更多完整式判成多余，`gmr doctor` 会开始报 `long-hand`。**那是对的**，
不要为了让报告安静而给这里加豁免。要豁免就说出理由，理由必须是一个分支。

判 `false` 的分支只会让 lint 少报，不会让它误报 —— 所以宁可漏，不可错杀。
新增分支时按这条来。

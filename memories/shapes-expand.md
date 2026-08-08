---
about: domains/coding/cli/src/shapes.rs#expand
watch: [sig, logic]
---

# 捕获规则必须先要求命中

展开出来的第一条规则是 `not exists(state.baseline) and obs.exact`，
第二条才是没命中时的 `absent`。**这个顺序是判据，不是风格。**

反过来写（先捕获、再检查命中）会让「坐标指向一个不存在的东西」静默成功：
探针在 `file` 命中、`name`/`heading` 没命中时不会报错，它退回同文件里的另一个候选，
`found: true` 而 `exact: false`。捕获规则如果不看 `exact`，就把那个**错的东西**
钉成基线，然后报 `settled` —— 从此它盯着错的对象，而且再也没有规则能触发，
因为 `baseline` 已经存在了。

这不是假设。本仓库的 `doctrine::red-cards` 就是这么坏了一整段历史：
它指向 CLAUDE.md 里一个在 `5f6b22d` 就被删掉的章节，退回到了文件第一个 heading，
跟 `doctrine::decisions` 的指纹一模一样（`bac58fed`，都在第 7 行），
一直显示安定。`fingerprint` shape 后来照这个顺序修好了。

`obs.found` 从来不是对的守卫 —— `found` 只是说「有某个 item 命中了」，
不是「标识这个东西的那个 item 命中了」。要用 `obs.exact`。

## 三段顺序，中间那段现在是推出来的

规则分三段：**两条开局 → 全部 `Now` 维 → 全部 `Since` 维 → 兜底 `true`**。

中间那段原来是硬编码的一条 `obs.exact == false => missing`。现在它是
`Reads::Now` 的维各生成一条 —— 但位置不变，而且**必须不变**：一个 `Now` 维成立
时说的是「这份读数不是关于我的目标的」，所以它的规则保留上一次好读数、把全部
`Since` 位原样带过去。让任何 `Since` 规则跑在它前面，就会拿另一个对象的读数去比
基线，把整个向量一次钉满。判据见 [[shapes-Dim]]。

## 变了要问什么

新加的规则插到了第一条前面 → 问它在 baseline 还不存在时会不会写出 baseline。
只要会，它就是第二个捕获入口，得跟第一条一样要求 `obs.exact`。

这个洞现在**结构上不可能**再出现：捕获规则不是手写的，是 `expand()` 生成的，
每个 shape 都只有这一个入口。两个手写规则表的 shape（`occurrence` 和 `symbol`，
用的是 `obs.found == false` 或者根本不检查）随双轨一起删掉了 —— 它们没有锚在用，
而留着就等于留着两条没人走、也没人检查的捕获路径。


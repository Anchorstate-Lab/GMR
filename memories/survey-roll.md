---
about: batteries/survey/src/matching.rs#roll
watch: [sig, logic]
---

# 名册比的是谁在场，不是他们长什么样

`roll` 是并列集合的身份表：一行一个，排过序。roster 拿它跟基线比，而不是比
全部候选的完整报告 —— 于是**改一个函数体不会让名册动**，因为身份没变。
这一维叫 `swapped`：有进有出而总数不变，只有它会亮（见 [[layers]]）。

**重复项保留**，所以 `roll.lines().count() == candidates` 恒成立。
去重会让「抽取器叫不出名字的候选」塌成同一个空行，名册就少数了而且不说话。
这不是假想：`layer::gmr` 的公开面全是 `pub use`，而 `use_declaration` 没有
`name` 字段，那五个候选一度全是空串（见 [[ast-signature]]）。

那次的修法是**给它们真名字**（`argument` 字段，也就是导入路径本身），
不是发明一个 `kind:@字节偏移` 兜底。字节偏移在它上方任何一次编辑后都会变，
那就是把 hair-trigger 换个地方重现一遍。

**一个在无关编辑下不稳定的 id，比丢掉这个候选更糟。两个都不是答案；
补上表示层的缺口才是。**

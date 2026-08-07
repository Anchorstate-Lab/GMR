---
about: crates/gmr-core/src/addr.rs#canonical_number_string
watch: [sig, logic]
---

# 数字的写法就是哈希的一部分

同一个数值有很多写法：`1.50` `1.5` `1.5e0` `-0`。规范化必须把它们收敛成一种，
否则「内容相同」的两份 JSON 会哈希成两个地址，`ContentHash` 就不再是内容的地址。

这里定死的四条：整数走 `to_string()` 不走浮点路径；`-0` 和 `-0.0` 一律写成 `0`；
小数尾部的 `0` 和孤零零的 `.` 去掉；`E` 一律小写成 `e`。

浮点用 ryu 格式化。**ryu 或 serde_json 悄悄改了格式，这个仓库全部历史哈希就对不上了** ——
测试 `canonical_form_is_pinned_against_library_drift` 把一个固定值的字节和哈希钉死，
就是为了让那种漂移在升级依赖时当场炸，而不是在某天比对旧日志时才发现。

结尾那个 `unreachable!` 不是偷懒：不开 `arbitrary_precision` 时
`serde_json::Number` 只有 PosInt / NegInt / Float 三种，`as_f64()` 对三种都是全函数。
开了那个 feature 这里就真的能走到，所以它是一条**依赖 feature 的不变量**。

## 变了要问什么

任何一条格式规则改了 → 全部历史 `ContentHash` 失效，日志比不回去。这不是「改进」，
是破坏性变更，得跟换探针版本一样对待。

`arbitrary_precision` 被某个依赖间接打开 → `unreachable!` 会 panic。
先问它为什么被打开，再决定是关掉还是给这里加一条真实分支。

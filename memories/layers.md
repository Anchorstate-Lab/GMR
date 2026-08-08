---
anchors:
  - key: layer::gmr-core
    probe: ast-map
    params: { root: crates/gmr-core }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr-expr
    probe: ast-map
    params: { root: crates/gmr-expr }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr-probe
    probe: ast-map
    params: { root: crates/gmr-probe }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr-store
    probe: ast-map
    params: { root: crates/gmr-store }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr-runtime
    probe: ast-map
    params: { root: crates/gmr-runtime }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr
    probe: ast-map
    params: { root: crates/gmr }
    position: { vis: "pub" }
    shape: roster
watch: [grew, shrank, roll]
---

# 一层的公开面变宽，就是这一层的职责变了

细颗粒度的锚回答「这段代码守什么」。它们再多也回答不了另一个问题：
**这一层现在是什么。** 一个 crate 多出一个 `pub`，没有任何单点锚会动 ——
而那恰恰是层与层之间的契约变了。

这六个锚盯的是六个 crate 的公开面名单。四个轴，`watch` 只订前三个：

```
grew     名单变长了        多出来的东西属于这一层吗
shrank   名单变短了        谁还在依赖走掉的
roll     成员换了人        有进有出，两个都要问
missing  坐标一个都没命中   锚指错了，或者这个 crate 没了
```

`grew` / `shrank` 量的是**相对基线的净方向**，不是「发生了什么」。加两个删一个只有
`grew` 亮 —— 那次删除靠 `roll` 说话，而具体是谁，要人去比 `baseline.roll` 和
`now.roll` 两份名单。求值器没有差集，这一步只能由人做，所以名单存的是**可读的名字**
而不是哈希：省空间的做法会把这个锚变成人无法回答的问题。

**这一层不看任何成员长什么样。** 改一个 pub 函数的实现、挪它的位置、换它的签名，
这四个轴一个都不会动。那些是细颗粒度锚的事，两层各管各的，删掉任何一层另一层都补不上。

## 名单变了，按这一层的入场判据问

| 层 | 只该有 | 一旦出现别的就问 |
|---|---|---|
| `gmr-core` | 词汇 · 内容地址 · Entry · fold | 它是不是开始知道「怎么取事实 / 怎么算规则 / 怎么存」 |
| `gmr-expr` | 纯表达式求值 | 有没有 IO · 时钟 · 对 gmr-core 的依赖 |
| `gmr-probe` | 探针调用契约 | 有没有混进具体传输实现 |
| `gmr-store` | 存储 trait 与 feature 门后的后端 | 新 trait 是按**可变性**切的吗（Journal / Binding / Sealer / Link / Queue） |
| `gmr-runtime` | 唯一编排层 | 它是不是开始替领域做判断 |
| `gmr` | 只 re-export | 出现任何自己的定义都是越界 |

判据出自 CLAUDE.md 的 crate 边界那一节。这里不复述它，只把它接到一个会自己开口的
锚上 —— 写在文档里的边界靠人记得去看，挂在锚上的边界会在越界那天把这张表递回来。

## 为什么是 `params` + `vis`，不是坐标

`about: <路径>` 只能指一个文件。整层要用完整式：`params: { root: <crate> }` 把探针的
视野缩到这个 crate，`position: { vis: "pub" }` 挑出公开面。这正是逃生口那四条里的
「探针要看的不是仓库根」，见 [[memories-lint]]。

`gmr` 那一层的名单几乎全是 `import:` —— 它的公开面**就是**那些 `pub use`。
这些条目一度没有身份（`use_declaration` 上没有 `name` 字段），修法是给它们真名字，
不是编一个稳定不了的兜底 ID。见 [[ast-signature]]。

## 变了要问什么

`watch` 里加了 `missing` → 想清楚：一个 crate 整个消失该由这个锚报，还是该关掉它。
默认不订，因为那通常意味着锚本身该改指别处。

`grew` 出现在 `gmr` 上 → 直接违反第 12 条，那一层只 re-export。

某一层的名单**大幅**变化 → 先问是不是拆包/改名。那种情况该开新一代锚并
`supersedes` 旧的（见 [[anchor-Superseded]]），不是把差异 accept 掉。

判据写成 `watch` 而不是写在这段散文里，是有意的：散文说错了没人会发现，
而 `watch` 里写错一个轴名 `sync` 当场报错。见 [[shapes-Dim]]。

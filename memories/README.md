---
anchors:
---

# 记忆的格式

一份记忆挂在锚上。锚机械地观测，记忆写的是**观测约束、但决定不了的那件事**。

## 正规形式 —— 只有这一种

```yaml
---
about: crates/gmr-core/src/addr.rs#write_array
watch: [sig, logic]
---

# 一句话说清这段代码在守什么

正文：这些维动了，该重新问什么。
```

| 字段 | 说什么 | 缺省 |
|---|---|---|
| `about` | 坐标 `路径` 或 `路径#名字`。**锚的 key 就是这个字符串** | 必填 |
| `watch` | 订阅哪几维，只影响递不递记忆和退出码 | shape 的内置订阅 |
| `shape` | 覆盖推断出来的形状 | 由坐标推 |

`about` 可以是一个列表 —— 一份记忆挂多个锚。

**`gmr anchor <坐标> -m "…"` 生成的就是这个形式。** 手写要跟它长一样。

## 坐标

`路径` → 整份名册（roster）。`路径#名字` → 那一个定义（contract）。

`名字` 指的是**能被单独指名的东西**：function · module · type。
不是它的调用点（那些在 `callee`），也不是类型的字段（那些在 `member`）——
见 [[ast-naming]]。所以 `路径#名字` 唯一，不会静默指错。

`const` 锚不了：`const_item` 不在 `lang.rs` 的 kinds 表里。要锚就锚它所在的函数。
`type` 的 `sig` 是它的成员（字段·变体·方法签名，见 [[ast-signature]]），`logic` 只剩成员的实现 ——
所以 struct 加一个字段是 `signature-changed`，不是 `logic-changed`；
而一个 struct 根本没有实现，`logic` 永不动。

## 逃生口

只有这几种情况用 `anchors:` 完整式，别的一律用上面那个：

- 要写 `rules` / `terminal`（手写规则表，第 7 条那个逃生口）
- 要写 `params`（探针要看的不是仓库根）
- 探针不吃 `路径#名字`（name-map · addr-map · 脚本探针）
- position 要写坐标语法表达不出来的键（`kind` · `member` · `nth`）

```yaml
anchors:
  - key: doctrine::decisions
    probe: prose-map
    position: { file: CLAUDE.md, heading: "…" }
    shape: fingerprint
```

裸键形式（`anchors: - some::key`）只**绑定**不**声明**，用它就得把声明另放一处。
本仓库不用它 —— `.anchor/anchors.toml` 已经删了，声明和记忆住在同一个文件里。

## 两条纪律

**代码里不写注释，一条都不写。** 要说的话写在这里，锚到那段代码上。注释和记忆
各存一份必然分家，而记忆有锚盯着、注释没有。

**不写代码里有的东西。** 名单、公开面、依赖清单由锚实时吐出来，`gmr status` 看得到。
机械可查的约束是 `gate.sh` 里可执行的检查。记忆写的是**为什么**，不是**是什么**。

## 谁守着这一节

`gmr doctor` 检查每一篇笔记，报三种：

| 码 | 说的是 | 退出码 |
|---|---|---|
| `unclaimed` | 没有 frontmatter —— 这篇笔记没有锚，没人观测它说的还成不成立 | 1 |
| `bare-key` | 裸键：只绑定不声明，而本仓库没有别处声明锚 | 1 |
| `long-hand` | 完整式写的东西坐标本来就能路由到，该退回 `about:` | 0（建议） |
| `retired` | 提到了 `shapes::RETIRED` 里的词 —— 这个 build 没有它了 | 0（建议） |

判据是**试着走一遍**：把 `key` 当坐标路由，如果推出来的探针和 position 跟手写的
一模一样，且没有 `rules` / `terminal` / 非默认 `params`，那这份完整式就没挣到它的位置。
所以上面「逃生口」那四条不是约定，是可执行的检查。

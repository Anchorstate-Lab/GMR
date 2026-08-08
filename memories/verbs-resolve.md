---
about:
  - domains/coding/cli/src/verbs/mod.rs#resolve_one
  - domains/coding/cli/src/verbs/mod.rs#pick
---

# 只读的动词把前缀摊开，会改状态的动词拒绝它

原来 CLI 把用户给的字符串直接 `AnchorKey::new`，不解析、不查存在性。三种坏输入
全部 exit 2，而且其中一种在说谎：

```
gmr check crates/gmr-core/src/addr.rs#write_aray
  → "the lease is held by someone else"
```

拼错一个字母，报的是租约冲突。成因是 `observe()` 先拿租约，队列里没有这一行就返回
`Leased`，永远走不到"没有这个锚"。**错误信息把我们的失败（找不到）说成了世界的
状态（别人在写）**，正好是 CLAUDE.md 那条 `NotFound 是世界答案 / ProbeError 是我们的
失败` 反过来。

## 前缀展开的分界线

`resolve` 摊开前缀，`resolve_one` 拒绝多于一个。分界不是「方便」，是**一份理由能不能
覆盖多个判断**：

| | 前缀 | 为什么 |
|---|---|---|
| `status` `read` `check` `observe` `health` | 摊开 | 看五个锚就是看五个锚，没有判断 |
| `close` `accept` `restate` `re*` `rebase` `requeue` | 拒绝 | 每一个都是独立判断，一份 `--why` 盖不住 |

这跟 `accept --all` 只肯配 `--criteria` 是同一条：一次声明变更是一个决定，
而每一处基线漂移各是各的。见 [[shapes-Dim]]。

## 变了要问什么

有动词从 `resolve_one` 换成 `resolve` → 问：它写不写日志？只要写，一次调用就会用
同一份理由封存多条记录，那是在伪造「我对这五个都做过判断」。

`nearest` 的排序换掉（现在是最长公共前缀）→ 只要拼错一个字母时正确的键不再排第一，
这个提示就白给了。测试 `a_typo_is_told_what_it_nearly_said` 钉的就是这一条。

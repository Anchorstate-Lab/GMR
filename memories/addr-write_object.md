---
about: crates/gmr-core/src/addr.rs#write_object
---

# 键排序就是内容地址的定义

`write_object` 先把 entries 按键排序再写。这一行是整个内容寻址的地基：
两个语义相同、键顺序不同的 JSON 必须哈希成同一个值，否则 `ContentHash`
就不是内容的地址，而是某次序列化的地址。

serde_json 不开 feature 时用 BTreeMap，看起来已经有序 —— 但那是**依赖 feature 的
巧合**，`preserve_order` 一开就变成插入序。

**而本仓库已经开着。** 工作区根的 `Cargo.toml` 直接写了
`serde_json = { version = "1", features = ["preserve_order"] }`，
`cargo tree -p gmr-core --format '{p} {f}'` 解析出来是 `default,indexmap,preserve_order,std`。
所以 `Map` 此刻**就是** IndexMap，迭代**就是**插入序 —— 这一行排序不是防着谁将来
打开什么，它是现在唯一撑着「`ContentHash` 是内容的地址」这句话的东西。

## 开着这个 feature，反而是测试看得见这一行的原因

风险方向跟直觉相反，两边都实测过：

```
preserve_order 开（今天）+ 删掉排序  → 5 个测试红
preserve_order 关       + 删掉排序  → 全绿，一个字不说
```

关掉之后 `Map` 变回 BTreeMap，迭代本来就有序，删不删这一行输出都一样 ——
`key_order_does_not_affect_output` · `nested_keys_sorted` ·
`content_hash_is_key_order_independent` · `whitespace_in_source_does_not_matter` ·
`canonical_form_is_pinned_against_library_drift` 五个全部照样过。代码于是**静默地**
改成依赖 BTreeMap 的迭代顺序，而那正是这一行存在的理由要躲开的耦合。

所以插入序不是这一行要防的敌人，是它的**证人**。

## 变了要问什么

排序被删掉、或者换成依赖 map 自身迭代顺序 → 不用问 `preserve_order` 开没开，
答案是开着，测试当场会红。

**真正要盯的是反过来那件事**：有人从工作区根的 `serde_json` 上摘掉
`preserve_order`（嫌它拖 indexmap、或者跟别的包对齐）。那一刻上面五个测试同时
失去分辨力，而它们**不会红** —— 一次让检查失效的改动，长得跟无害的依赖清理
一模一样。摘之前先问：内容寻址还剩谁在看着？

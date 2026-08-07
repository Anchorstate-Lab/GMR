---
about: crates/gmr-core/src/addr.rs#write_object
---

# 键排序就是内容地址的定义

`write_object` 先把 entries 按键排序再写。这一行是整个内容寻址的地基：
两个语义相同、键顺序不同的 JSON 必须哈希成同一个值，否则 `ContentHash`
就不是内容的地址，而是某次序列化的地址。

serde_json 默认用 BTreeMap，看起来已经有序 —— 但那是**依赖 feature 的巧合**
（开了 `preserve_order` 就变成插入序）。这里显式排序，是不让哈希的稳定性
挂在一个别人可以打开的 feature 上。

## 变了要问什么

排序被删掉、或者换成依赖 map 自身迭代顺序 → 立刻问：`preserve_order` 有没有被
某个依赖间接打开？这类漂移不会有任何测试失败，只会让历史哈希对不上。

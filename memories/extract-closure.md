---
about:
  - domains/coding/extract/build.rs#gmr_outcome_contract
  - domains/coding/extract/build.rs#locked_versions
watch: [sig, logic]
---

# 闭包不能依赖它自己要保证的东西

## build 脚本链不了它正在构建的那个 crate

所以 `gmr_outcome_contract()` 手写着 `"gmr.outcome.v1"`，跟 `gmr_core::OUTCOME_CONTRACT`
靠 `lib.rs` 里的测试对齐。这是一份**明知故犯的第二份**，唯一的理由是构建期够不着第一份 ——
而看住它的那个测试跟它同时落地，不是以后再说。

## Cargo.lock 手工解析

`locked_versions` 自己拆 `[[package]]`，不用 TOML 解析器。因为闭包的作用就是
「能改变输出的全部输入都进哈希」，而拉一个解析器进来就等于让哈希依赖那个解析器的版本 ——
它一升级，全仓探针版本翻篇，而没有任何一次观测的结果变过。

同理，`SHARED`（`matching.rs` · `walk.rs`）是整文件读进哈希的。**碰这两个文件一个字节 ——
加个常量、加个测试、删行注释 —— 四个抽取器全换版本，要一次 `gmr rebase --all`。**
所以碰它们的事必须编进同一个 commit、同一次迁移。这个哈希宁可多报也不漏报。

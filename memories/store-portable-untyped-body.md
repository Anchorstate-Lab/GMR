---
about: crates/gmr-store/tests/portable.rs#export_does_not_require_the_body_to_match_this_builds_entry_enum
watch: [logic]
---

# Export has to survive a body this build's own `Entry` enum cannot parse

This test inserts a journal row whose `body` is JSON that no version of
`gmr_core::Entry` could deserialize (`"entry":"some_future_variant"`) and
checks that `export_jsonl` still emits it verbatim. That is the whole point
of keeping row bodies as `serde_json::Value` instead of round-tripping
through the typed `Entry`/`Binding` (see the module doc at the top of
`portable.rs`): a file the *old* binary wrote has to stay exportable by a
*new* binary whose `Entry` enum has grown a variant since, and the reverse
direction across an upgrade is exactly when a typed round-trip would fail.

## When this changes, ask

Does `export_jsonl` or `import_jsonl` gain a step that deserializes a row's
`body` into `Entry` (or `Binding`) rather than passing the raw
`serde_json::Value` through? That reintroduces exactly the coupling this
test exists to catch — an old export would then fail to round-trip through
a newer binary.

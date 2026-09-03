---
about:
  - crates/gmr-store/src/sqlite/portable.rs#EXPORT_SCHEMA
  - crates/gmr-store/src/sqlite/portable.rs#Line
watch: [sig]
---

# The export format versions itself independently of the SQL schema

`EXPORT_SCHEMA` is a separate identity from `schema::SCHEMA_VERSION` on
purpose: the SQL schema can change for reasons that do not touch what a
JSONL row looks like, and a row shape can in principle change without the
SQL schema moving. Bump `EXPORT_SCHEMA` only when a `Line` variant's shape
changes, never just because the SQL schema bumped.

`Line` is tagged (`#[serde(tag = "table", ...)]`) so a single stream can
carry the manifest plus all eight tables (journal, bindings, binding_anchors,
binding_revocations, binding_revoked_tags, links, link_revocations, sealed)
without needing a second file format or a multi-file bundle — each line says
which table it belongs to.

Adding a variant does **not** bump `EXPORT_SCHEMA`, and the binding
revocations landed without one: an old file simply lacks the new lines and
replays fine, which is the direction that matters (§9.5 — surviving upgrades
is the reason the file exists). A new file read by an old build fails its
tagged parse, which is the refusal that direction deserves. Only a change to
an existing variant's shape — something that makes an old file parse
differently — moves the version.

## When this changes, ask

Did a `Line` variant's fields change in a way an old export file would no
longer parse the same way? That is exactly what `EXPORT_SCHEMA` exists to
flag — bump it, and `import_jsonl`'s schema check (see
[[store-portable-import]]) will refuse mismatched files instead of
half-parsing them.

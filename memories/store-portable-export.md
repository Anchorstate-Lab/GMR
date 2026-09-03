---
about: crates/gmr-store/src/sqlite/portable.rs#export_jsonl
watch: [sig, logic]
---

# Export order is fixed so every foreign key appears before the row that names it

`export_jsonl` always writes tables in the same order — journal, bindings,
binding_anchors, binding_revocations, binding_revoked_tags, links,
link_revocations, sealed — because `import_jsonl` replays the stream in a
single pass and needs every row a later line references to already exist. A
`binding_anchors` row names a `bindings.seq`; a `bindings` row's
`bound_at_seq` names a `journal.seq`; a `binding_revoked_tags` row names both
a `binding_revocations.seq` and a `bindings.seq`; a `link_revocations` row
names a `links.seq`. Writing (or replaying) any of them out of this order
would mean the referencing row lands before the row it refers to.

The binding revocations were not always here: the first year of this format
carried the link side's revocations and dropped the binding side's, so a
retired conclusion or a detached memory came back alive after a migration —
a judgment its owner had already made, undone by the crossing. The
round-trip test now revokes before exporting precisely so that omission
cannot return.

## When this changes, ask

Does a new table's row reference another table's `seq`? If so it has to be
written after that table in this fixed order, or `import_jsonl` needs a
second pass to resolve forward references — which it currently does not
have.

---
about: crates/gmr-store/src/sqlite/portable.rs#expect_seq
watch: [sig, logic]
---

# A `seq` landing anywhere but where the export recorded it is a bug, made loud

`expect_seq` compares `expected` (what the row held when the export was
written) against `landed` (where the `INSERT ... RETURNING seq` actually
put it during import). They can only differ if the destination table was
not really empty when import started — and `import_jsonl`'s pre-flight
count check (see [[store-portable-import]]) is supposed to make that
impossible, turning "not empty" into a refusal before any row is inserted.
So a mismatch here means that guarantee already failed somewhere upstream.

Without this check the mismatch would not vanish — it would surface later
as a silently wrong foreign key: a `Bindings` row's `bound_at_seq`, or a
`BindingAnchors` row's `seq`, pointing at whatever sequence number actually
landed in that slot rather than the one the export intended. `expect_seq`
turns that into a loud, immediate `StoreError::corrupt` instead.

## When this changes, ask

Does every table whose rows carry a `seq` that another row references
(`bindings`, `binding_anchors`) still call `expect_seq` right after its
insert? Skipping the check on any of them reopens the silent-misalignment
failure mode this function exists to close.

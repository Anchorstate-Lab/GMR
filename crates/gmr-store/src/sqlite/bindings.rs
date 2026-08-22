use crate::{Asserted, BindingRecord, BindingStore, Sealer, StoreError};
use async_trait::async_trait;
use gmr_core::{AnchorKey, Binding, ContentHash, Ref, Seq, Source, Version, content_hash_of_bytes};
use sqlx::{Row, SqlitePool};

use super::{db_err, decode_err, ref_key};

pub struct SqliteBindings {
    pub(super) pool: SqlitePool,
}

impl SqliteBindings {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BindingStore for SqliteBindings {
    async fn bind(&self, asserted: &Asserted) -> Result<(), StoreError> {
        let binding = &asserted.binding;
        let body = serde_json::to_string(binding)
            .map_err(|e| StoreError::other(format!("could not serialise the binding: {e}")))?;

        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let seq: i64 = sqlx::query_scalar(
            "INSERT INTO bindings (reference, body, bound_version, bound_at_seq, source, asserted_at, baseline_at_seq) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL) RETURNING seq",
        )
        .bind(ref_key(&binding.reference))
        .bind(&body)
        .bind(asserted.bound_version.as_str())
        .bind(asserted.bound_at_seq.map(|s| s as i64))
        .bind(asserted.source.as_str())
        .bind(asserted.at.to_rfc3339())
        .fetch_one(&mut *tx)
        .await
        .map_err(db_err)?;

        for anchor in &binding.anchors {
            sqlx::query("INSERT INTO binding_anchors (seq, anchor) VALUES (?1, ?2)")
                .bind(seq)
                .bind(anchor.as_str())
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    async fn bindings_on(&self, anchor: &AnchorKey) -> Result<Vec<BindingRecord>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT b.body, b.bound_version, b.bound_at_seq, b.source, b.asserted_at FROM bindings b
            JOIN binding_anchors ba ON ba.seq = b.seq
            WHERE ba.anchor = ?1
              AND b.seq = (SELECT MAX(seq) FROM bindings WHERE reference = b.reference)
            ORDER BY b.seq
            "#,
        )
        .bind(anchor.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        decode_all(rows)
    }

    async fn binding_of(&self, reference: &Ref) -> Result<Option<BindingRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT body, bound_version, bound_at_seq, source, asserted_at FROM bindings WHERE reference = ?1 ORDER BY seq DESC LIMIT 1",
        )
        .bind(ref_key(reference))
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        row.map(decode_one).transpose()
    }

    async fn all(&self) -> Result<Vec<BindingRecord>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT b.body, b.bound_version, b.bound_at_seq, b.source, b.asserted_at FROM bindings b
            WHERE b.seq = (SELECT MAX(seq) FROM bindings WHERE reference = b.reference)
            ORDER BY b.seq
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        decode_all(rows)
    }
}

#[async_trait]
impl Sealer for SqliteBindings {
    async fn seal(&self, bytes: &[u8]) -> Result<ContentHash, StoreError> {
        let address = content_hash_of_bytes(bytes);
        sqlx::query("INSERT OR IGNORE INTO sealed (address, body) VALUES (?1, ?2)")
            .bind(address.as_str())
            .bind(bytes)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(address)
    }

    async fn sealed(&self, address: &ContentHash) -> Result<Option<Vec<u8>>, StoreError> {
        let row = sqlx::query("SELECT body FROM sealed WHERE address = ?1")
            .bind(address.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(row.map(|r| r.get::<Vec<u8>, _>("body")))
    }
}

fn decode_one(row: sqlx::sqlite::SqliteRow) -> Result<BindingRecord, StoreError> {
    let binding: Binding =
        serde_json::from_str(&row.get::<String, _>("body")).map_err(decode_err)?;
    let bound_version = row
        .get::<Option<String>, _>("bound_version")
        .map(Version::new)
        .ok_or_else(|| {
            StoreError::other(
                "this assertion carries no version, meaning no fetch has ever answered for \
                 the record it names. The column allows that and this build does not yet \
                 produce it; a database holding one was written by a later generation",
            )
        })?;
    let raw = row.get::<String, _>("source");
    let source = Source::parse(&raw).ok_or_else(|| {
        StoreError::other(format!(
            "`{raw}` is not a source this build knows. A row whose source cannot be read \
             cannot be weighed, and guessing one would invent the fact a reader relies on"
        ))
    })?;
    Ok(BindingRecord {
        binding,
        bound_version,
        bound_at_seq: row.get::<Option<i64>, _>("bound_at_seq").map(|s| s as Seq),
        source,
        asserted_at: row
            .get::<Option<String>, _>("asserted_at")
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
            .map(|t| t.with_timezone(&chrono::Utc)),
    })
}

fn decode_all(rows: Vec<sqlx::sqlite::SqliteRow>) -> Result<Vec<BindingRecord>, StoreError> {
    rows.into_iter().map(decode_one).collect()
}

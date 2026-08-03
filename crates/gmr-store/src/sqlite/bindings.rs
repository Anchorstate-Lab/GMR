use crate::{BindingRecord, BindingStore, Sealer, StoreError};
use async_trait::async_trait;
use gmr_core::{AnchorKey, Binding, ContentHash, Ref, Version, content_hash_of_bytes};
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
    async fn bind(&self, binding: &Binding, bound_version: &Version) -> Result<(), StoreError> {
        let body = serde_json::to_string(binding)
            .map_err(|e| StoreError::other(format!("could not serialise the binding: {e}")))?;

        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let seq: i64 = sqlx::query_scalar(
            "INSERT INTO bindings (reference, body, bound_version) VALUES (?1, ?2, ?3) RETURNING seq",
        )
        .bind(ref_key(&binding.reference))
        .bind(&body)
        .bind(bound_version.as_str())
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
            SELECT b.body, b.bound_version FROM bindings b
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
            "SELECT body, bound_version FROM bindings WHERE reference = ?1 ORDER BY seq DESC LIMIT 1",
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
            SELECT b.body, b.bound_version FROM bindings b
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
    Ok(BindingRecord {
        binding,
        bound_version: Version::new(row.get::<String, _>("bound_version")),
    })
}

fn decode_all(rows: Vec<sqlx::sqlite::SqliteRow>) -> Result<Vec<BindingRecord>, StoreError> {
    rows.into_iter().map(decode_one).collect()
}

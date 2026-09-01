use crate::{Asserted, BindingRecord, BindingStore, Revocation, Sealer, StoreError};
use async_trait::async_trait;
use gmr_core::{
    AnchorKey, Binding, Claim, ContentHash, FactAddress, Seq, Source, Version,
    content_hash_of_bytes,
};
use sqlx::{Row, SqlitePool};

use super::{claim_key, db_err, decode_err};

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
            "INSERT INTO bindings (reference, body, bound_version, bound_at_seq, source, asserted_at, baseline_at_seq, saw) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7) RETURNING seq",
        )
        .bind(claim_key(&binding.claim))
        .bind(&body)
        .bind(asserted.bound_version.as_ref().map(Version::as_str))
        .bind(asserted.bound_at_seq.map(|s| s as i64))
        .bind(asserted.source.as_str())
        .bind(asserted.at.to_rfc3339())
        .bind(looked(&asserted.saw))
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

    async fn revoke(&self, revocation: &Revocation) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let seq: i64 = sqlx::query_scalar(
            "INSERT INTO binding_revocations (reference, anchor, source, revoked_at) \
             VALUES (?1, ?2, ?3, ?4) RETURNING seq",
        )
        .bind(claim_key(&revocation.claim))
        .bind(revocation.at.as_str())
        .bind(revocation.source.as_str())
        .bind(revocation.when.to_rfc3339())
        .fetch_one(&mut *tx)
        .await
        .map_err(db_err)?;

        for tag in &revocation.tags {
            sqlx::query(
                "INSERT OR IGNORE INTO binding_revoked_tags (revocation, binding, anchor) \
                 VALUES (?1, ?2, ?3)",
            )
            .bind(seq)
            .bind(tag.binding as i64)
            .bind(tag.anchor.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    async fn bindings_on(&self, anchors: &[AnchorKey]) -> Result<Vec<BindingRecord>, StoreError> {
        if anchors.is_empty() {
            return Ok(Vec::new());
        }
        let slots = placeholders(anchors.len(), 1);
        let sql = format!(
            r#"
            SELECT b.seq, b.body, b.bound_version, b.bound_at_seq, b.source, b.asserted_at, b.saw,
                   ba.anchor AS live_anchor
            FROM bindings b
            JOIN binding_anchors ba ON ba.seq = b.seq
            WHERE ba.anchor IN ({slots})
              AND NOT EXISTS (
                  SELECT 1 FROM binding_revoked_tags rt
                  JOIN binding_revocations r ON r.seq = rt.revocation
                  WHERE rt.binding = b.seq AND rt.anchor = ba.anchor
                    AND r.anchor IN ({slots})
              )
            ORDER BY b.seq, ba.anchor
            "#
        );
        let mut q = sqlx::query(&sql);
        for anchor in anchors {
            q = q.bind(anchor.as_str());
        }
        gathered(q.fetch_all(&self.pool).await.map_err(db_err)?)
    }

    async fn binding_of(&self, claim: &Claim) -> Result<Vec<BindingRecord>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT b.seq, b.body, b.bound_version, b.bound_at_seq, b.source, b.asserted_at, b.saw,
                   ba.anchor AS live_anchor
            FROM bindings b
            LEFT JOIN binding_anchors ba ON ba.seq = b.seq
              AND NOT EXISTS (
                  SELECT 1 FROM binding_revoked_tags rt
                  WHERE rt.binding = b.seq AND rt.anchor = ba.anchor
              )
            WHERE b.reference = ?1
            ORDER BY b.seq, ba.anchor
            "#,
        )
        .bind(claim_key(claim))
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        gathered(rows)
    }

    async fn all(&self) -> Result<Vec<BindingRecord>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT b.seq, b.body, b.bound_version, b.bound_at_seq, b.source, b.asserted_at, b.saw,
                   ba.anchor AS live_anchor
            FROM bindings b
            LEFT JOIN binding_anchors ba ON ba.seq = b.seq
              AND NOT EXISTS (
                  SELECT 1 FROM binding_revoked_tags rt
                  WHERE rt.binding = b.seq AND rt.anchor = ba.anchor
              )
            ORDER BY b.seq, ba.anchor
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        gathered(rows)
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

fn looked(saw: &std::collections::BTreeSet<FactAddress>) -> Option<String> {
    match saw.len() {
        0 => None,
        1 => saw.iter().next().map(|a| a.as_str().to_owned()),
        _ => serde_json::to_string(saw).ok(),
    }
}

fn seen(held: Option<&str>) -> Result<std::collections::BTreeSet<FactAddress>, StoreError> {
    let Some(text) = held else {
        return Ok(Default::default());
    };
    let spelled: Vec<String> = match text.starts_with('[') {
        true => serde_json::from_str(text).map_err(decode_err)?,
        false => vec![text.to_owned()],
    };
    spelled
        .into_iter()
        .map(FactAddress::try_new)
        .collect::<Result<_, _>>()
        .map_err(|e| {
            StoreError::other(format!(
                "`saw` holds something that is not a fact address: {e}. It is what says which \
                 reading the asserter was actually looking at, and a value nothing can match \
                 is worse than none"
            ))
        })
}

fn placeholders(n: usize, from: usize) -> String {
    (from..from + n)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn gathered(rows: Vec<sqlx::sqlite::SqliteRow>) -> Result<Vec<BindingRecord>, StoreError> {
    let mut out: Vec<BindingRecord> = Vec::new();
    for row in rows {
        let seq = row.get::<i64, _>("seq") as Seq;
        let live = row
            .get::<Option<String>, _>("live_anchor")
            .map(AnchorKey::new);
        match out.last_mut() {
            Some(last) if last.seq == seq => {
                last.binding.anchors.extend(live);
                continue;
            }
            _ => {}
        }
        let mut record = decode_one(seq, row)?;
        record.binding.anchors = live.into_iter().collect();
        out.push(record);
    }
    Ok(out)
}

fn decode_one(seq: Seq, row: sqlx::sqlite::SqliteRow) -> Result<BindingRecord, StoreError> {
    let binding: Binding =
        serde_json::from_str(&row.get::<String, _>("body")).map_err(decode_err)?;
    let bound_version = row
        .get::<Option<String>, _>("bound_version")
        .map(Version::new);
    let raw = row.get::<String, _>("source");
    let source = Source::parse(&raw).ok_or_else(|| {
        StoreError::other(format!(
            "`{raw}` is not a source this build knows. A row whose source cannot be read \
             cannot be weighed, and guessing one would invent the fact a reader relies on"
        ))
    })?;
    Ok(BindingRecord {
        seq,
        binding,
        bound_version,
        bound_at_seq: row.get::<Option<i64>, _>("bound_at_seq").map(|s| s as Seq),
        saw: seen(row.get::<Option<String>, _>("saw").as_deref())?,
        source,
        asserted_at: row
            .get::<Option<String>, _>("asserted_at")
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
            .map(|t| t.with_timezone(&chrono::Utc)),
    })
}

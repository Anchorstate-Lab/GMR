use crate::{LinkRecord, LinkRevocation, LinkStore, StoreError};
use async_trait::async_trait;
use gmr_core::{LinkKind, Ref, Source};
use sqlx::Row;

use super::bindings::SqliteBindings;
use super::{db_err, decode_err, ref_key};

#[async_trait]
impl LinkStore for SqliteBindings {
    async fn link(
        &self,
        from: &Ref,
        to: &Ref,
        kind: LinkKind,
        source: Source,
    ) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO links (from_ref, to_ref, kind, source) VALUES (?1, ?2, ?3, ?4)")
            .bind(ref_key(from))
            .bind(ref_key(to))
            .bind(&kind.0)
            .bind(source.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn unlink(&self, revocation: &LinkRevocation) -> Result<u64, StoreError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let mut sql = String::from(
            "SELECT l.seq FROM links l \
             WHERE l.from_ref = ?1 AND l.to_ref = ?2 AND l.kind = ?3 \
             AND NOT EXISTS (SELECT 1 FROM link_revocations r WHERE r.link = l.seq)",
        );
        if revocation.asserted_as.is_some() {
            sql.push_str(" AND l.source = ?4");
        }
        let mut query = sqlx::query(&sql)
            .bind(ref_key(&revocation.from))
            .bind(ref_key(&revocation.to))
            .bind(&revocation.kind.0);
        if let Some(of) = revocation.asserted_as {
            query = query.bind(of.as_str());
        }
        let rows = query.fetch_all(&mut *tx).await.map_err(db_err)?;

        let mut revoked = 0u64;
        for row in &rows {
            sqlx::query(
                "INSERT INTO link_revocations (link, source, revoked_at) VALUES (?1, ?2, ?3)",
            )
            .bind(row.get::<i64, _>("seq"))
            .bind(revocation.source.as_str())
            .bind(revocation.when.to_rfc3339())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
            revoked += 1;
        }
        tx.commit().await.map_err(db_err)?;
        Ok(revoked)
    }

    async fn links_of(&self, reference: &Ref) -> Result<Vec<LinkRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT l.to_ref, l.kind, l.source FROM links l \
             WHERE l.from_ref = ?1 \
             AND NOT EXISTS (SELECT 1 FROM link_revocations r WHERE r.link = l.seq) \
             ORDER BY l.seq",
        )
        .bind(ref_key(reference))
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        rows.into_iter()
            .map(|r| {
                let to: Ref =
                    serde_json::from_str(&r.get::<String, _>("to_ref")).map_err(decode_err)?;
                let raw: String = r.get("source");
                let source = Source::parse(&raw).ok_or_else(|| {
                    StoreError::corrupt(format!(
                        "a link carries the source `{raw}`, which this build does not know"
                    ))
                })?;
                Ok(LinkRecord {
                    to,
                    kind: LinkKind(r.get("kind")),
                    source,
                })
            })
            .collect()
    }
}

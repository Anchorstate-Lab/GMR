use crate::{LinkStore, StoreError};
use async_trait::async_trait;
use gmr_core::{Link, LinkKind, Ref};
use sqlx::Row;

use super::bindings::SqliteBindings;
use super::{db_err, decode_err, ref_key};

#[async_trait]
impl LinkStore for SqliteBindings {
    async fn link(&self, from: &Ref, to: &Ref, kind: LinkKind) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO links (from_ref, to_ref, kind) VALUES (?1, ?2, ?3)")
            .bind(ref_key(from))
            .bind(ref_key(to))
            .bind(&kind.0)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn links_of(&self, reference: &Ref) -> Result<Vec<Link>, StoreError> {
        let rows = sqlx::query("SELECT to_ref, kind FROM links WHERE from_ref = ?1 ORDER BY seq")
            .bind(ref_key(reference))
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;

        rows.into_iter()
            .map(|r| {
                let to: Ref =
                    serde_json::from_str(&r.get::<String, _>("to_ref")).map_err(decode_err)?;
                Ok(Link {
                    to,
                    kind: LinkKind(r.get("kind")),
                })
            })
            .collect()
    }
}

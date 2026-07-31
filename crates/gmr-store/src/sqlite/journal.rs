use crate::{Fence, Journal, StoreError};
use async_trait::async_trait;
use gmr_core::{AnchorKey, Entry, Seq};
use sqlx::{Row, SqlitePool};

use super::{db_err, decode_err};

pub struct SqliteJournal {
    pool: SqlitePool,
}

impl SqliteJournal {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Journal for SqliteJournal {
    async fn append(
        &self,
        anchor: &AnchorKey,
        entry: &Entry,
        fence: Fence,
    ) -> Result<Seq, StoreError> {
        let body = serde_json::to_string(entry)
            .map_err(|e| StoreError::other(format!("条目序列化失败：{e}")))?;

        let mut tx = self.pool.begin().await.map_err(db_err)?;

        let seen: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(fence), 0) FROM journal WHERE anchor = ?1")
                .bind(anchor.as_str())
                .fetch_one(&mut *tx)
                .await
                .map_err(db_err)?;

        if fence.0 > 0 && (fence.0 as i64) < seen {
            return Err(StoreError::constraint(format!(
                "fencing 令牌 {} 已过期（已见 {seen}）—— 租约到期不等于持有者停工",
                fence.0
            )));
        }

        let seq: i64 = sqlx::query_scalar(
            "INSERT INTO journal (anchor, fence, body) VALUES (?1, ?2, ?3) RETURNING seq",
        )
        .bind(anchor.as_str())
        .bind(fence.0 as i64)
        .bind(&body)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_err)?;

        tx.commit().await.map_err(db_err)?;
        Ok(seq as Seq)
    }

    async fn entries(
        &self,
        anchor: &AnchorKey,
        from: Seq,
    ) -> Result<Vec<(Seq, Entry)>, StoreError> {
        let rows = sqlx::query(
            "SELECT seq, body FROM journal WHERE anchor = ?1 AND seq >= ?2 ORDER BY seq",
        )
        .bind(anchor.as_str())
        .bind(from as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        rows.into_iter()
            .map(|r| {
                let seq: i64 = r.get("seq");
                let body: String = r.get("body");
                let entry = serde_json::from_str(&body).map_err(decode_err)?;
                Ok((seq as Seq, entry))
            })
            .collect()
    }

    async fn anchors(&self) -> Result<Vec<AnchorKey>, StoreError> {
        let rows = sqlx::query("SELECT DISTINCT anchor FROM journal ORDER BY anchor")
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|r| AnchorKey::new(r.get::<String, _>("anchor")))
            .collect())
    }
}

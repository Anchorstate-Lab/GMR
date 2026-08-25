use crate::{Chained, Fence, Journal, StoreError};
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
            .map_err(|e| StoreError::other(format!("could not serialise the entry: {e}")))?;

        let mut held = self.pool.acquire().await.map_err(db_err)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *held)
            .await
            .map_err(db_err)?;

        let appended = appended(&mut held, anchor, entry, fence, &body).await;
        let closed = sqlx::query(match appended.is_ok() {
            true => "COMMIT",
            false => "ROLLBACK",
        })
        .execute(&mut *held)
        .await;

        let seq = appended?;
        closed.map_err(db_err)?;
        Ok(seq)
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

    async fn head(&self) -> Result<Seq, StoreError> {
        let row = sqlx::query("SELECT COALESCE(MAX(seq), 0) AS head FROM journal")
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(row.get::<i64, _>("head") as Seq)
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

async fn appended(
    held: &mut sqlx::SqliteConnection,
    anchor: &AnchorKey,
    entry: &Entry,
    fence: Fence,
    body: &str,
) -> Result<Seq, StoreError> {
    let seen: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(fence), 0) FROM journal WHERE anchor = ?1")
            .bind(anchor.as_str())
            .fetch_one(&mut *held)
            .await
            .map_err(db_err)?;

    crate::journal::guard(fence, seen, entry)?;

    let prev: Option<String> = sqlx::query_scalar("SELECT hash FROM journal ORDER BY seq DESC")
        .fetch_optional(&mut *held)
        .await
        .map_err(db_err)?
        .flatten();

    let hash = crate::journal::link(prev.as_deref(), anchor, fence, entry)?;

    let seq: i64 = sqlx::query_scalar(
        "INSERT INTO journal (anchor, fence, body, prev, hash) VALUES (?1, ?2, ?3, ?4, ?5)
         RETURNING seq",
    )
    .bind(anchor.as_str())
    .bind(fence.epoch().unwrap_or(0) as i64)
    .bind(body)
    .bind(prev.as_deref())
    .bind(hash.as_str())
    .fetch_one(&mut *held)
    .await
    .map_err(db_err)?;

    Ok(seq as Seq)
}

#[async_trait]
impl Chained for SqliteJournal {
    async fn chain_break(&self) -> Result<Option<Seq>, StoreError> {
        let rows =
            sqlx::query("SELECT seq, anchor, fence, body, prev, hash FROM journal ORDER BY seq")
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;

        let mut carried: Option<String> = None;
        for r in rows {
            let seq: i64 = r.get("seq");
            let stored: Option<String> = r.get("hash");
            let Some(stored) = stored else {
                carried = None;
                continue;
            };
            let prev: Option<String> = r.get("prev");
            if prev != carried {
                return Ok(Some(seq as Seq));
            }
            let anchor = AnchorKey::new(r.get::<String, _>("anchor"));
            let fence = match r.get::<i64, _>("fence") {
                0 => Fence::Unleased,
                n => Fence::Held(n as u64),
            };
            let entry: Entry =
                serde_json::from_str(&r.get::<String, _>("body")).map_err(decode_err)?;
            if crate::journal::link(prev.as_deref(), &anchor, fence, &entry)?.as_str() != stored {
                return Ok(Some(seq as Seq));
            }
            carried = Some(stored);
        }
        Ok(None)
    }
}

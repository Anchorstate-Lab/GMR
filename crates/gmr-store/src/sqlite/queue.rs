use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use gmr_core::AnchorKey;
use sqlx::{Row, SqlitePool};

use super::db_err;
use crate::queue::{Disposition, Queue, Ticket};
use crate::{Fence, StoreError};

pub struct SqliteQueue {
    pub(super) pool: SqlitePool,
}

impl SqliteQueue {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Queue for SqliteQueue {
    async fn enqueue(&self, anchor: &AnchorKey, due: DateTime<Utc>) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO queue (anchor, due, lease_until, parked) VALUES (?1, ?2, 0, 0)
             ON CONFLICT(anchor) DO UPDATE SET due = ?2, lease_until = 0, parked = 0",
        )
        .bind(anchor.as_str())
        .bind(due.timestamp())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn ensure_enqueued(
        &self,
        anchor: &AnchorKey,
        due: DateTime<Utc>,
    ) -> Result<bool, StoreError> {
        let row = sqlx::query(
            "INSERT INTO queue (anchor, due, lease_until, parked) VALUES (?1, ?2, 0, 0)
             ON CONFLICT(anchor) DO NOTHING
             RETURNING anchor",
        )
        .bind(anchor.as_str())
        .bind(due.timestamp())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.is_some())
    }

    async fn due(
        &self,
        now: DateTime<Utc>,
        lease: Duration,
        limit: usize,
    ) -> Result<Vec<Ticket>, StoreError> {
        let until = now + lease;
        let rows = sqlx::query(
            "UPDATE queue SET lease_until = ?1, epoch = epoch + 1
             WHERE anchor IN (
                 SELECT anchor FROM queue
                 WHERE parked = 0 AND due <= ?2 AND lease_until <= ?2
                 ORDER BY due LIMIT ?3
             )
             RETURNING anchor, epoch",
        )
        .bind(until.timestamp())
        .bind(now.timestamp())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(rows
            .into_iter()
            .map(|r| Ticket {
                anchor: AnchorKey::new(r.get::<String, _>("anchor")),
                fence: Fence::Held(r.get::<i64, _>("epoch") as u64),
                lease_until: until,
            })
            .collect())
    }

    async fn lease(
        &self,
        anchor: &AnchorKey,
        now: DateTime<Utc>,
        lease: Duration,
    ) -> Result<Option<Ticket>, StoreError> {
        let until = now + lease;
        let row = sqlx::query(
            "UPDATE queue SET lease_until = ?1, epoch = epoch + 1
             WHERE anchor = ?2 AND lease_until <= ?3
             RETURNING epoch",
        )
        .bind(until.timestamp())
        .bind(anchor.as_str())
        .bind(now.timestamp())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(row.map(|r| Ticket {
            anchor: anchor.clone(),
            fence: Fence::Held(r.get::<i64, _>("epoch") as u64),
            lease_until: until,
        }))
    }

    async fn settle(
        &self,
        ticket: &Ticket,
        disposition: Disposition,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        match disposition {
            Disposition::Retire => {
                sqlx::query("UPDATE queue SET parked = 1, lease_until = 0 WHERE anchor = ?1")
                    .bind(ticket.anchor.as_str())
                    .execute(&self.pool)
                    .await
                    .map_err(db_err)?;
            }
            Disposition::Reschedule { after_secs } | Disposition::Backoff { after_secs } => {
                sqlx::query("UPDATE queue SET due = ?2, lease_until = 0 WHERE anchor = ?1")
                    .bind(ticket.anchor.as_str())
                    .bind((now + Duration::seconds(after_secs)).timestamp())
                    .execute(&self.pool)
                    .await
                    .map_err(db_err)?;
            }
        }
        Ok(())
    }
}

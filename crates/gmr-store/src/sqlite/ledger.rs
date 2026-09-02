use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::Row;

use super::db_err;
use super::queue::SqliteQueue;
use super::sightings::moment;
use crate::StoreError;
use crate::ledger::{Ledger, Spending};

#[async_trait]
impl Ledger for SqliteQueue {
    async fn spent(
        &self,
        session: &str,
        verb: &str,
        bytes: u64,
        at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO ledger (session, verb, calls, bytes, last_at) VALUES (?1, ?2, 1, ?3, ?4)
             ON CONFLICT(session, verb)
             DO UPDATE SET calls = calls + 1, bytes = bytes + ?3, last_at = ?4",
        )
        .bind(session)
        .bind(verb)
        .bind(bytes as i64)
        .bind(at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn spending(&self) -> Result<Vec<Spending>, StoreError> {
        let rows = sqlx::query(
            "SELECT session, verb, calls, bytes, last_at FROM ledger ORDER BY session, verb",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.into_iter()
            .map(|r| {
                Ok(Spending {
                    session: r.get("session"),
                    verb: r.get("verb"),
                    calls: r.get::<i64, _>("calls") as u64,
                    bytes: r.get::<i64, _>("bytes") as u64,
                    last_at: moment(r.get::<Option<String>, _>("last_at"))?,
                })
            })
            .collect()
    }
}

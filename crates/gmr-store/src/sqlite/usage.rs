use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gmr_core::Claim;
use sqlx::Row;

use super::queue::SqliteQueue;
use super::sightings::moment;
use super::{claim_key, db_err, decode_err};
use crate::StoreError;
use crate::usage::{Usage, Used};

#[async_trait]
impl Usage for SqliteQueue {
    async fn used(&self, claim: &Claim, at: DateTime<Utc>) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO usage (claim, count, last_at) VALUES (?1, 1, ?2)
             ON CONFLICT(claim) DO UPDATE SET count = count + 1, last_at = ?2",
        )
        .bind(claim_key(claim))
        .bind(at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn usage_of(&self, claim: &Claim) -> Result<Used, StoreError> {
        let row = sqlx::query("SELECT count, last_at FROM usage WHERE claim = ?1")
            .bind(claim_key(claim))
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;

        let Some(r) = row else {
            return Ok(Used::default());
        };
        Ok(Used {
            count: r.get::<i64, _>("count") as u64,
            last_at: moment(r.get::<Option<String>, _>("last_at"))?,
        })
    }

    async fn all_usage(&self) -> Result<Vec<(Claim, Used)>, StoreError> {
        let rows = sqlx::query("SELECT claim, count, last_at FROM usage ORDER BY claim")
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        rows.into_iter()
            .map(|r| {
                let spelled: String = r.get("claim");
                let claim: Claim = serde_json::from_str(&spelled).map_err(decode_err)?;
                Ok((
                    claim,
                    Used {
                        count: r.get::<i64, _>("count") as u64,
                        last_at: moment(r.get::<Option<String>, _>("last_at"))?,
                    },
                ))
            })
            .collect()
    }
}

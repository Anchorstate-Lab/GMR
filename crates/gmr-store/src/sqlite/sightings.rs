use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gmr_core::AnchorKey;
use sqlx::Row;

use super::db_err;
use super::queue::SqliteQueue;
use crate::{Seen, Sightings, StoreError};

pub(crate) fn moment(text: Option<String>) -> Result<Option<DateTime<Utc>>, StoreError> {
    text.map(|t| {
        DateTime::parse_from_rfc3339(&t)
            .map(|d| d.with_timezone(&Utc))
            .map_err(|e| {
                StoreError::corrupt(format!(
                    "sighting.last_at holds {t:?}, which is not a time: {e}"
                ))
            })
    })
    .transpose()
}

#[async_trait]
impl Sightings for SqliteQueue {
    async fn sighted(&self, anchor: &AnchorKey, at: DateTime<Utc>) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO sighting (anchor, count, last_at) VALUES (?1, 1, ?2)
             ON CONFLICT(anchor) DO UPDATE SET count = count + 1, last_at = ?2",
        )
        .bind(anchor.as_str())
        .bind(at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn seen(&self, anchor: &AnchorKey) -> Result<Seen, StoreError> {
        let row = sqlx::query("SELECT count, last_at FROM sighting WHERE anchor = ?1")
            .bind(anchor.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;

        let Some(r) = row else {
            return Ok(Seen::default());
        };
        Ok(Seen {
            sightings: r.get::<i64, _>("count") as u64,
            last_at: moment(r.get::<Option<String>, _>("last_at"))?,
        })
    }

    async fn all_seen(&self) -> Result<BTreeMap<AnchorKey, Seen>, StoreError> {
        let rows = sqlx::query("SELECT anchor, count, last_at FROM sighting")
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;

        rows.into_iter()
            .map(|r| {
                Ok((
                    AnchorKey::new(r.get::<String, _>("anchor")),
                    Seen {
                        sightings: r.get::<i64, _>("count") as u64,
                        last_at: moment(r.get::<Option<String>, _>("last_at"))?,
                    },
                ))
            })
            .collect()
    }
}

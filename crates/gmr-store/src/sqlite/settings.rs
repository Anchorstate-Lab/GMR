use async_trait::async_trait;
use gmr_core::{AnchorKey, Recorded, Retain, RunSettings};
use sqlx::Row;

use super::db_err;
use super::queue::SqliteQueue;
use crate::{Settings, StoreError};

#[async_trait]
impl Settings for SqliteQueue {
    async fn put(&self, anchor: &AnchorKey, settings: &RunSettings) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO settings (anchor, retain, cadence_secs, budget_ms, facts)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(anchor) DO UPDATE SET
                 retain = ?2, cadence_secs = ?3, budget_ms = ?4, facts = ?5",
        )
        .bind(anchor.as_str())
        .bind(match settings.retain {
            Retain::Tick => "tick",
            Retain::Full => "full",
        })
        .bind(settings.cadence_secs.map(|s| s as i64))
        .bind(settings.budget_ms.map(|s| s as i64))
        .bind(match settings.facts {
            Recorded::Plain => "plain",
            Recorded::Digests => "digests",
        })
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn get(&self, anchor: &AnchorKey) -> Result<Option<RunSettings>, StoreError> {
        let row = sqlx::query(
            "SELECT retain, cadence_secs, budget_ms, facts FROM settings WHERE anchor = ?1",
        )
        .bind(anchor.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        let Some(r) = row else { return Ok(None) };
        let retain = match r.get::<String, _>("retain").as_str() {
            "full" => Retain::Full,
            "tick" => Retain::Tick,
            other => {
                return Err(StoreError::corrupt(format!(
                    "settings.retain holds {other:?}, which is not a retention"
                )));
            }
        };
        let facts = match r.get::<String, _>("facts").as_str() {
            "plain" => Recorded::Plain,
            "digests" => Recorded::Digests,
            other => {
                return Err(StoreError::corrupt(format!(
                    "settings.facts holds {other:?}, which is not a recording"
                )));
            }
        };
        Ok(Some(RunSettings {
            retain,
            facts,
            cadence_secs: r.get::<Option<i64>, _>("cadence_secs").map(|s| s as u64),
            budget_ms: r.get::<Option<i64>, _>("budget_ms").map(|s| s as u64),
        }))
    }
}

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use gmr_core::{AnchorKey, RunSettings};
use gmr_store::{Disposition, Queue, Settings, Ticket};

use crate::error::RuntimeError;
use crate::policy::Policy;

pub struct Scheduler {
    queue: Option<Arc<dyn Queue>>,
    settings: Arc<dyn Settings>,
    policy: Policy,
}

impl Scheduler {
    pub(crate) fn new(
        queue: Option<Arc<dyn Queue>>,
        settings: Arc<dyn Settings>,
        policy: Policy,
    ) -> Self {
        Self {
            queue,
            settings,
            policy,
        }
    }

    pub async fn settings_for(&self, anchor: &AnchorKey) -> Result<RunSettings, RuntimeError> {
        Ok(self.settings.get(anchor).await?.unwrap_or_default())
    }

    pub async fn set_settings(
        &self,
        anchor: &AnchorKey,
        settings: &RunSettings,
    ) -> Result<(), RuntimeError> {
        Ok(self.settings.put(anchor, settings).await?)
    }

    pub async fn budget_for(&self, anchor: &AnchorKey) -> Result<Option<u64>, RuntimeError> {
        Ok(self.settings_for(anchor).await?.budget_ms)
    }

    pub async fn cadence_for(&self, anchor: &AnchorKey) -> Result<i64, RuntimeError> {
        Ok(self
            .settings_for(anchor)
            .await?
            .cadence_secs
            .unwrap_or(self.policy.cadence_secs) as i64)
    }

    pub fn leases_configured(&self) -> bool {
        self.queue.is_some()
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    pub async fn ensure_enqueued(
        &self,
        anchor: &AnchorKey,
        due: DateTime<Utc>,
    ) -> Result<bool, RuntimeError> {
        let Some(queue) = self.queue.as_ref() else {
            return Ok(false);
        };
        Ok(queue.ensure_enqueued(anchor, due).await?)
    }

    pub async fn requeue_now(
        &self,
        anchor: &AnchorKey,
        due: DateTime<Utc>,
    ) -> Result<bool, RuntimeError> {
        let Some(queue) = self.queue.as_ref() else {
            return Ok(false);
        };
        queue.enqueue(anchor, due).await?;
        Ok(true)
    }

    pub async fn due(
        &self,
        now: DateTime<Utc>,
        lease: Duration,
        limit: usize,
    ) -> Result<Vec<Ticket>, RuntimeError> {
        let queue = self.queue.as_ref().ok_or(RuntimeError::NoQueue)?;
        Ok(queue.due(now, lease, limit).await?)
    }

    pub async fn lease(
        &self,
        anchor: &AnchorKey,
        now: DateTime<Utc>,
        lease: Duration,
    ) -> Result<Option<Ticket>, RuntimeError> {
        let queue = self.queue.as_ref().ok_or(RuntimeError::NoQueue)?;
        Ok(queue.lease(anchor, now, lease).await?)
    }

    pub async fn settle(
        &self,
        ticket: &Ticket,
        disposition: Disposition,
        now: DateTime<Utc>,
    ) -> Result<(), RuntimeError> {
        let queue = self.queue.as_ref().ok_or(RuntimeError::NoQueue)?;
        Ok(queue.settle(ticket, disposition, now).await?)
    }
}

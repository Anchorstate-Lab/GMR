use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use gmr_core::AnchorKey;
use gmr_store::{Disposition, Queue, Ticket};

use crate::error::RuntimeError;
use crate::policy::Policy;

/// The queue and the policy numbers governing it. A deployment without a queue
/// is legal; the lease path is then unavailable.
pub struct Scheduler {
    queue: Option<Arc<dyn Queue>>,
    policy: Policy,
}

impl Scheduler {
    pub(crate) fn new(queue: Option<Arc<dyn Queue>>, policy: Policy) -> Self {
        Self { queue, policy }
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

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use gmr_core::AnchorKey;
use gmr_store::{Disposition, Queue, Ticket};

use crate::error::RuntimeError;
use crate::policy::Policy;

/// The queue and the policy numbers that govern it. A deployment without a
/// queue is legal — `pass`/`observe`'s lease path is then simply unavailable,
/// which the rest of the runtime treats as "poll it by hand instead."
pub struct Scheduler {
    queue: Option<Arc<dyn Queue>>,
    policy: Policy,
}

impl Scheduler {
    pub(crate) fn new(queue: Option<Arc<dyn Queue>>, policy: Policy) -> Self {
        Self { queue, policy }
    }

    pub fn has_lease(&self) -> bool {
        self.queue.is_some()
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    /// `Ok(true)` if actually enqueued, `Ok(false)` if this deployment has no
    /// queue — that is not a failure, just a no-op.
    pub async fn enqueue(
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

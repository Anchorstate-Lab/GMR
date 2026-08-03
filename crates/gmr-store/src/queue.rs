use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use gmr_core::AnchorKey;

use crate::error::StoreError;
use crate::journal::Fence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub anchor: AnchorKey,
    pub fence: Fence,
    pub lease_until: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Reschedule { after_secs: i64 },
    Backoff { after_secs: i64 },
    Retire,
}

/// Implementations must guarantee: **fences issued for one anchor increase
/// strictly monotonically, and retiring does not reset the counter.** The journal
/// uses it as a high-water mark to block stale-lease writes; going backwards once
/// wedges that anchor forever.
#[async_trait]
pub trait Queue: Send + Sync {
    /// Unconditionally (re)schedules `anchor`, clearing any lease and parked
    /// state — even if it was backing off or retired. A deliberate reset, not
    /// something a routine sync should do to every anchor on every run.
    async fn enqueue(&self, anchor: &AnchorKey, due: DateTime<Utc>) -> Result<(), StoreError>;

    /// Schedules `anchor` only if it has no row yet. `Ok(true)` if a row was
    /// inserted, `Ok(false)` if one already existed — in which case it is left
    /// untouched, backoff/park state included.
    async fn ensure_enqueued(
        &self,
        anchor: &AnchorKey,
        due: DateTime<Utc>,
    ) -> Result<bool, StoreError>;

    async fn due(
        &self,
        now: DateTime<Utc>,
        lease: Duration,
        limit: usize,
    ) -> Result<Vec<Ticket>, StoreError>;

    /// Take the lease on one specific anchor, due or not.
    ///
    /// Hand-triggered observations come through here — otherwise they can only
    /// write past the token, which is the very second writer the lease prevents.
    /// Not getting it means someone else holds it, so let them write.
    async fn lease(
        &self,
        anchor: &AnchorKey,
        now: DateTime<Utc>,
        lease: Duration,
    ) -> Result<Option<Ticket>, StoreError>;

    async fn settle(
        &self,
        ticket: &Ticket,
        disposition: Disposition,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError>;
}

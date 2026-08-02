use async_trait::async_trait;
use gmr_core::{AnchorKey, Entry, Seq};

use crate::error::{ErrorCode, ErrorKind, StoreError};

/// Write token.
///
/// A lease expiring does not mean the holder stopped working, so the journal has
/// to be able to refuse writes carrying a stale token. `Held` is the epoch one
/// lease issued; `Unleased` is a deployment without leases, where there is no
/// second writer to speak of.
///
/// An enum rather than "0 means none": an in-band sentinel lets callers conflate
/// "I hold no token" with "my token is 0", and those two should be refused in
/// opposite ways.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fence {
    Unleased,
    Held(u64),
}

impl Fence {
    pub fn epoch(self) -> Option<u64> {
        match self {
            Self::Unleased => None,
            Self::Held(n) => Some(n),
        }
    }
}

#[async_trait]
pub trait Journal: Send + Sync {
    async fn append(
        &self,
        anchor: &AnchorKey,
        entry: &Entry,
        fence: Fence,
    ) -> Result<Seq, StoreError>;

    async fn entries(&self, anchor: &AnchorKey, from: Seq)
    -> Result<Vec<(Seq, Entry)>, StoreError>;

    async fn anchors(&self) -> Result<Vec<AnchorKey>, StoreError>;
}

/// Token check. Both backends share this one — written separately they would
/// sooner or later be wrong separately.
pub fn guard(fence: Fence, seen: i64, entry: &Entry) -> Result<(), StoreError> {
    match fence {
        Fence::Held(epoch) if (epoch as i64) < seen => Err(StoreError::with_code(
            ErrorKind::Constraint,
            ErrorCode::StaleFence,
            format!(
                "fencing token {epoch} is stale (already saw {seen}) — a lease expiring \
             does not mean the holder stopped working"
            ),
        )),
        // Observing is the lease's job. Once an anchor is under lease management,
        // no observation may be slipped in beside it — that is exactly the second
        // writer the lease exists to prevent. Author revisions are exempt.
        Fence::Unleased if seen > 0 && entry.is_sighting() => Err(StoreError::with_code(
            ErrorKind::Constraint,
            ErrorCode::LeaseManagedObservation,
            "observations on this anchor are lease-managed and will not be accepted \
             without a token; go through the queue, or stop polling"
                .to_owned(),
        )),
        _ => Ok(()),
    }
}

use async_trait::async_trait;
use gmr_core::{AnchorKey, ContentHash, Entry, Seq, content_hash_of};

use crate::error::{ErrorCode, ErrorKind, StoreError};

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

    async fn head(&self) -> Result<Seq, StoreError>;
}

#[async_trait]
pub trait Chained: Send + Sync {
    async fn chain_break(&self) -> Result<Option<Seq>, StoreError>;
}

pub fn link(
    prev: Option<&str>,
    anchor: &AnchorKey,
    fence: Fence,
    entry: &Entry,
) -> Result<ContentHash, StoreError> {
    let entry = serde_json::to_value(entry)
        .map_err(|e| StoreError::other(format!("could not shape the entry to link it: {e}")))?;
    content_hash_of(&serde_json::json!({
        "prev": prev,
        "anchor": anchor.as_str(),
        "fence": fence.epoch().unwrap_or(0),
        "entry": entry,
    }))
    .map_err(|e| StoreError::other(format!("could not link the entry: {e}")))
}

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

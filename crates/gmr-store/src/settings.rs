use async_trait::async_trait;
use gmr_core::{AnchorKey, RunSettings};

use crate::error::StoreError;

/// **Mutable, and deliberately so.** These are operating knobs, not criteria:
/// changing one rewrites nothing that was already judged, so it needs no
/// sealed rationale and no append-only history.
#[async_trait]
pub trait Settings: Send + Sync {
    async fn put(&self, anchor: &AnchorKey, settings: &RunSettings) -> Result<(), StoreError>;

    /// `None` when nothing was ever set, leaving the caller to apply the
    /// deployment default.
    async fn get(&self, anchor: &AnchorKey) -> Result<Option<RunSettings>, StoreError>;
}

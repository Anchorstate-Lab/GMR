use async_trait::async_trait;
use gmr_core::{AnchorKey, Entry, Seq};

use crate::error::StoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Fence(pub u64);

impl Fence {
    pub const NONE: Fence = Fence(0);
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

use std::sync::Arc;

use gmr_core::{AnchorKey, Entry, Seq};
use gmr_store::{Fence, Journal};

use crate::error::RuntimeError;

/// The append-only anchor log. Every verb that only needs to read or write
/// entries goes through here — it does not carry the capability to invoke a
/// probe, touch a memory binding, or reach the queue.
pub struct AnchorLog {
    journal: Arc<dyn Journal>,
}

impl AnchorLog {
    pub(crate) fn new(journal: Arc<dyn Journal>) -> Self {
        Self { journal }
    }

    pub async fn entries(
        &self,
        key: &AnchorKey,
        from: Seq,
    ) -> Result<Vec<(Seq, Entry)>, RuntimeError> {
        Ok(self.journal.entries(key, from).await?)
    }

    pub async fn append(
        &self,
        key: &AnchorKey,
        entry: &Entry,
        fence: Fence,
    ) -> Result<Seq, RuntimeError> {
        Ok(self.journal.append(key, entry, fence).await?)
    }

    pub async fn anchors(&self) -> Result<Vec<AnchorKey>, RuntimeError> {
        Ok(self.journal.anchors().await?)
    }
}

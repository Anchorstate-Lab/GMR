use async_trait::async_trait;
use gmr_core::{AnchorKey, RunSettings};

use crate::error::StoreError;

#[async_trait]
pub trait Settings: Send + Sync {
    async fn put(&self, anchor: &AnchorKey, settings: &RunSettings) -> Result<(), StoreError>;

    async fn get(&self, anchor: &AnchorKey) -> Result<Option<RunSettings>, StoreError>;
}

use async_trait::async_trait;
use gmr_core::ContentHash;

use crate::error::StoreError;

#[async_trait]
pub trait Sealer: Send + Sync {
    async fn seal(&self, bytes: &[u8]) -> Result<ContentHash, StoreError>;

    async fn sealed(&self, address: &ContentHash) -> Result<Option<Vec<u8>>, StoreError>;
}

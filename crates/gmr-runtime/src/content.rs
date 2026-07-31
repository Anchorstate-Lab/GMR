use async_trait::async_trait;
use gmr_core::{ExternalId, ProviderId, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub version: Version,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct ContentError {
    pub message: String,
}

impl ContentError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[async_trait]
pub trait ContentProvider: Send + Sync {
    fn provider(&self) -> &ProviderId;

    async fn fetch(&self, id: &ExternalId) -> Result<Option<Fetched>, ContentError>;

    async fn fetch_at(
        &self,
        id: &ExternalId,
        version: &Version,
    ) -> Result<Option<Vec<u8>>, ContentError>;
}

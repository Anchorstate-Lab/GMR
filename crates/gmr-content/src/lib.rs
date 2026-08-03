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
    pub code: ContentErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentErrorCode {
    ProviderFailed,
}

impl ContentErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProviderFailed => "provider_failed",
        }
    }
}

impl ContentError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            code: ContentErrorCode::ProviderFailed,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code.as_str()
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

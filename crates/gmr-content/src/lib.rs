//! Two contracts, one required and one not.
//!
//! `ContentProvider` is how a reference becomes bytes and a version; every
//! store that GMR can bind to implements it. `History` and `MemorySource`
//! are capabilities, and a store that lacks one simply does not implement
//! it — no flag, no variant every caller has to handle.
//!
//! Nothing in the base calls `MemorySource`. It lives here so that a
//! battery and a domain can agree on the shape of "what records are in
//! this store" without either one owning the vocabulary. Which store to
//! enumerate, and how much of it, is a domain decision.

use async_trait::async_trait;
use gmr_core::{ExternalId, ProviderId, Ref, Version};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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

    fn history(&self) -> Option<&dyn History> {
        None
    }
}

#[async_trait]
pub trait History: Send + Sync {
    async fn fetch_at(
        &self,
        id: &ExternalId,
        version: &Version,
    ) -> Result<Option<Vec<u8>>, ContentError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claim {
    Says(serde_json::Value),
    Silent,
    Malformed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub reference: Ref,
    pub version: Version,
    pub bytes: Vec<u8>,
    pub claim: Claim,
}

#[async_trait]
pub trait MemorySource: Send + Sync {
    fn provider(&self) -> &ProviderId;

    async fn list(&self) -> Result<Vec<Record>, ContentError>;
}

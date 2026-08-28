#[cfg(feature = "testkit")]
pub mod testkit;

use async_trait::async_trait;
use gmr_budget::Budget;
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
    BudgetSpent,
}

impl ContentErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProviderFailed => "provider_failed",
            Self::BudgetSpent => "budget_spent",
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

    pub fn spent(message: impl Into<String>) -> Self {
        Self {
            code: ContentErrorCode::BudgetSpent,
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

    async fn fetch(
        &self,
        id: &ExternalId,
        budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError>;

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
        budget: &Budget,
    ) -> Result<Option<Vec<u8>>, ContentError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub reference: Ref,
    pub version: Version,
    pub bytes: Vec<u8>,
}

#[async_trait]
pub trait MemorySource: Send + Sync {
    fn provider(&self) -> &ProviderId;

    async fn list(&self, budget: &Budget) -> Result<Vec<Record>, ContentError>;
}

pub struct MemoryStore {
    content: std::sync::Arc<dyn ContentProvider>,
    source: Option<std::sync::Arc<dyn MemorySource>>,
}

impl MemoryStore {
    pub fn new(content: std::sync::Arc<dyn ContentProvider>) -> Self {
        Self {
            content,
            source: None,
        }
    }

    pub fn listing(mut self, source: std::sync::Arc<dyn MemorySource>) -> Self {
        self.source = Some(source);
        self
    }

    pub fn provider(&self) -> &ProviderId {
        self.content.provider()
    }

    pub fn content(&self) -> std::sync::Arc<dyn ContentProvider> {
        std::sync::Arc::clone(&self.content)
    }

    pub fn source(&self) -> Option<&dyn MemorySource> {
        self.source.as_deref()
    }
}

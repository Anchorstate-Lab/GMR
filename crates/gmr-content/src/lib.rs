//! One contract every store owes, and three capabilities it may not have.
//!
//! `ContentProvider` is how a reference becomes bytes and a version; every
//! store that GMR can bind to implements it. `History`, `MemorySource` and
//! `Declaring` are capabilities, and a store that lacks one simply does not
//! implement it — no flag, no variant every caller has to handle.
//!
//! Nothing in the base calls `MemorySource` or `Declaring`. They live here
//! so that a battery and a domain can agree on the shape of "what records
//! are in this store" without either one owning the vocabulary. Which store
//! to enumerate, and how much of it, is a domain decision.
//!
//! `Declaring` is synchronous and takes no `Budget`. That is the whole of
//! its admission test: a store reachable only over a network cannot
//! implement it.

#[cfg(feature = "testkit")]
pub mod testkit;

use async_trait::async_trait;
use gmr_core::{ExternalId, ProviderId, Ref, Version};
use gmr_probe::Budget;

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
}

#[async_trait]
pub trait MemorySource: Send + Sync {
    fn provider(&self) -> &ProviderId;

    async fn list(&self, budget: &Budget) -> Result<Vec<Record>, ContentError>;
}

pub trait Declaring: Send + Sync {
    fn provider(&self) -> &ProviderId;

    fn records(&self) -> Result<Vec<Record>, ContentError>;

    fn claim_of(&self, record: &Record) -> Claim;

    fn name_of(&self, reference: &Ref) -> Option<String>;
}

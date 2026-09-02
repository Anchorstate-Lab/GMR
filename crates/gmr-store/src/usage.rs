use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gmr_core::Claim;

use crate::error::StoreError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Used {
    pub count: u64,
    pub last_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait Usage: Send + Sync {
    async fn used(&self, claim: &Claim, at: DateTime<Utc>) -> Result<(), StoreError>;

    async fn usage_of(&self, claim: &Claim) -> Result<Used, StoreError>;

    async fn all_usage(&self) -> Result<Vec<(Claim, Used)>, StoreError>;
}

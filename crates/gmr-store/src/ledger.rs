use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spending {
    pub session: String,
    pub verb: String,
    pub calls: u64,
    pub bytes: u64,
    pub last_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait Ledger: Send + Sync {
    async fn spent(
        &self,
        session: &str,
        verb: &str,
        bytes: u64,
        at: DateTime<Utc>,
    ) -> Result<(), StoreError>;

    async fn spending(&self) -> Result<Vec<Spending>, StoreError>;
}

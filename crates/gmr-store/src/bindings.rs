use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gmr_core::{AnchorKey, Binding, Ref, Seq, Source, Version};

use crate::error::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRecord {
    pub binding: Binding,
    pub bound_version: Version,
    pub bound_at_seq: Option<Seq>,
    pub source: Source,
    pub asserted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asserted {
    pub binding: Binding,
    pub bound_version: Version,
    pub bound_at_seq: Option<Seq>,
    pub source: Source,
    pub at: DateTime<Utc>,
}

#[async_trait]
pub trait BindingStore: Send + Sync {
    async fn bind(&self, asserted: &Asserted) -> Result<(), StoreError>;

    async fn bindings_on(&self, anchor: &AnchorKey) -> Result<Vec<BindingRecord>, StoreError>;

    async fn binding_of(&self, reference: &Ref) -> Result<Option<BindingRecord>, StoreError>;

    async fn all(&self) -> Result<Vec<BindingRecord>, StoreError>;
}

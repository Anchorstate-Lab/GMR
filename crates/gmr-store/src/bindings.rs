use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gmr_core::{AnchorKey, Binding, Ref, Seq, Source, Version};

use crate::error::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRecord {
    pub seq: Seq,
    pub binding: Binding,
    pub bound_version: Option<Version>,
    pub bound_at_seq: Option<Seq>,
    pub source: Source,
    pub asserted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tag {
    pub binding: Seq,
    pub anchor: AnchorKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revocation {
    pub reference: Ref,
    pub at: AnchorKey,
    pub tags: Vec<Tag>,
    pub source: Source,
    pub when: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asserted {
    pub binding: Binding,
    pub bound_version: Option<Version>,
    pub bound_at_seq: Option<Seq>,
    pub source: Source,
    pub at: DateTime<Utc>,
}

#[async_trait]
pub trait BindingStore: Send + Sync {
    async fn bind(&self, asserted: &Asserted) -> Result<(), StoreError>;

    async fn revoke(&self, revocation: &Revocation) -> Result<(), StoreError>;

    async fn bindings_on(&self, anchors: &[AnchorKey]) -> Result<Vec<BindingRecord>, StoreError>;

    async fn binding_of(&self, reference: &Ref) -> Result<Vec<BindingRecord>, StoreError>;

    async fn all(&self) -> Result<Vec<BindingRecord>, StoreError>;
}

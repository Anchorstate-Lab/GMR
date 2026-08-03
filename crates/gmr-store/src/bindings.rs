use async_trait::async_trait;
use gmr_core::{AnchorKey, Binding, Ref, Version};

use crate::error::StoreError;

/// A `Binding` plus the store-layer metadata that goes with a particular
/// write of it: which content version was current when this relation was
/// declared. Separate from `Binding` itself because that metadata is a
/// property of the write event, not of the `reference x anchors` relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRecord {
    pub binding: Binding,
    pub bound_version: Version,
}

#[async_trait]
pub trait BindingStore: Send + Sync {
    async fn bind(&self, binding: &Binding, bound_version: &Version) -> Result<(), StoreError>;

    async fn bindings_on(&self, anchor: &AnchorKey) -> Result<Vec<BindingRecord>, StoreError>;

    async fn binding_of(&self, reference: &Ref) -> Result<Option<BindingRecord>, StoreError>;

    async fn all(&self) -> Result<Vec<BindingRecord>, StoreError>;
}

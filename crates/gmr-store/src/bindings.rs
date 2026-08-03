use async_trait::async_trait;
use gmr_core::{AnchorKey, Binding, Ref, Seq, Version};

use crate::error::StoreError;

/// A `Binding` plus metadata about the write that recorded it, which is not
/// part of the `reference x anchors` relation itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRecord {
    pub binding: Binding,
    pub bound_version: Version,
    /// The bound anchor's head at bind time; `None` unless exactly one anchor.
    pub bound_at_seq: Option<Seq>,
}

#[async_trait]
pub trait BindingStore: Send + Sync {
    async fn bind(
        &self,
        binding: &Binding,
        bound_version: &Version,
        bound_at_seq: Option<Seq>,
    ) -> Result<(), StoreError>;

    async fn bindings_on(&self, anchor: &AnchorKey) -> Result<Vec<BindingRecord>, StoreError>;

    async fn binding_of(&self, reference: &Ref) -> Result<Option<BindingRecord>, StoreError>;

    async fn all(&self) -> Result<Vec<BindingRecord>, StoreError>;
}

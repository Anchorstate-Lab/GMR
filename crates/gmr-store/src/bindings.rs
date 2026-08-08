use async_trait::async_trait;
use gmr_core::{AnchorKey, Binding, Ref, Seq, Version};

use crate::error::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRecord {
    pub binding: Binding,
    pub bound_version: Version,
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

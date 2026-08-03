use async_trait::async_trait;
use gmr_core::{AnchorKey, Binding, Ref, Seq, Version};

use crate::error::StoreError;

/// A `Binding` plus the store-layer metadata that goes with a particular
/// write of it: which content version was current when this relation was
/// declared, and where the log stood at that moment. Separate from `Binding`
/// itself because that metadata is a property of the write event, not of the
/// `reference x anchors` relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRecord {
    pub binding: Binding,
    pub bound_version: Version,
    /// The bound anchor's journal head when this record was written — `None`
    /// when the binding names zero or several anchors, where "which anchor's
    /// head" has no single answer. Lets a reader ask "has the anchor moved
    /// since this was bound" without a second, separately-tracked cursor.
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

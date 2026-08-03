use async_trait::async_trait;
use gmr_core::{AnchorKey, Binding, Ref};

use crate::error::StoreError;

#[async_trait]
pub trait BindingStore: Send + Sync {
    async fn bind(&self, binding: &Binding) -> Result<(), StoreError>;

    async fn bindings_on(&self, anchor: &AnchorKey) -> Result<Vec<Binding>, StoreError>;

    async fn binding_of(&self, reference: &Ref) -> Result<Option<Binding>, StoreError>;

    async fn all(&self) -> Result<Vec<Binding>, StoreError>;
}

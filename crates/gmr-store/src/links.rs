use async_trait::async_trait;
use gmr_core::{Link, LinkKind, Ref};

use crate::error::StoreError;

#[async_trait]
pub trait LinkStore: Send + Sync {
    async fn link(&self, from: &Ref, to: &Ref, kind: LinkKind) -> Result<(), StoreError>;

    async fn links_of(&self, reference: &Ref) -> Result<Vec<Link>, StoreError>;
}

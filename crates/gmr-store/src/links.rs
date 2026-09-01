use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gmr_core::{LinkKind, Ref, Source};

use crate::error::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRecord {
    pub to: Ref,
    pub kind: LinkKind,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRevocation {
    pub from: Ref,
    pub to: Ref,
    pub kind: LinkKind,
    pub asserted_as: Option<Source>,
    pub source: Source,
    pub when: DateTime<Utc>,
}

#[async_trait]
pub trait LinkStore: Send + Sync {
    async fn link(
        &self,
        from: &Ref,
        to: &Ref,
        kind: LinkKind,
        source: Source,
    ) -> Result<(), StoreError>;

    async fn unlink(&self, revocation: &LinkRevocation) -> Result<u64, StoreError>;

    async fn links_of(&self, reference: &Ref) -> Result<Vec<LinkRecord>, StoreError>;

    async fn all(&self) -> Result<Vec<(Ref, LinkRecord)>, StoreError>;
}

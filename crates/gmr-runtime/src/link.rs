use gmr_core::{Link, LinkKind, Ref};

use crate::assembly::Runtime;
use crate::error::RuntimeError;

impl Runtime {
    pub async fn link(&self, from: &Ref, to: &Ref, kind: LinkKind) -> Result<(), RuntimeError> {
        self.memory.link(from, to, kind).await
    }

    pub async fn links_of(&self, reference: &Ref) -> Result<Vec<Link>, RuntimeError> {
        self.memory.links_of(reference).await
    }
}

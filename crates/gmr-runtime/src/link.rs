use gmr_core::{Link, LinkKind, Ref};

use crate::assembly::Runtime;
use crate::error::RuntimeError;

impl Runtime {
    /// Records that `from` relates to `to`. Independent of anchoring: linking
    /// two references says nothing about which anchors either is bound to.
    pub async fn link(&self, from: &Ref, to: &Ref, kind: LinkKind) -> Result<(), RuntimeError> {
        self.links.link(from, to, kind).await?;
        Ok(())
    }

    pub async fn links_of(&self, reference: &Ref) -> Result<Vec<Link>, RuntimeError> {
        Ok(self.links.links_of(reference).await?)
    }
}

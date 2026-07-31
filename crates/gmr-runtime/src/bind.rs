use gmr_core::{AnchorKey, Binding, Link, Ref, Version};

use crate::assembly::Runtime;
use crate::error::RuntimeError;

impl Runtime {
    pub async fn bind(
        &self,
        reference: Ref,
        anchors: Vec<AnchorKey>,
        bound_version: Version,
        links: Vec<Link>,
    ) -> Result<(), RuntimeError> {
        self.bindings
            .bind(&Binding {
                reference,
                anchors,
                bound_version,
                links,
            })
            .await?;
        Ok(())
    }

    pub async fn bindings_on(&self, anchor: &AnchorKey) -> Result<Vec<Binding>, RuntimeError> {
        Ok(self.bindings.bindings_on(anchor).await?)
    }
}

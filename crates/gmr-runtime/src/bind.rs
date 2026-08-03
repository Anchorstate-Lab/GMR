use gmr_core::{AnchorKey, Binding, Ref, Version};
use gmr_store::BindingRecord;

use crate::assembly::Runtime;
use crate::error::RuntimeError;

impl Runtime {
    pub async fn bind(
        &self,
        reference: Ref,
        anchors: Vec<AnchorKey>,
        bound_version: Version,
    ) -> Result<(), RuntimeError> {
        self.bindings
            .bind(&Binding { reference, anchors }, &bound_version)
            .await?;
        Ok(())
    }

    pub async fn bindings_on(
        &self,
        anchor: &AnchorKey,
    ) -> Result<Vec<BindingRecord>, RuntimeError> {
        Ok(self.bindings.bindings_on(anchor).await?)
    }
}

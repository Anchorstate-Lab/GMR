use chrono::Utc;
use gmr_core::{AnchorKey, Binding, Ref, Source, Version};
use gmr_store::BindingRecord;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;

impl Runtime {
    pub async fn bind(
        &self,
        reference: Ref,
        anchors: Vec<AnchorKey>,
        bound_version: Version,
        source: Source,
    ) -> Result<(), RuntimeError> {
        self.memory
            .bind(
                &self.log,
                &Binding { reference, anchors },
                &bound_version,
                source,
                Utc::now(),
            )
            .await
    }

    pub async fn bindings_on(
        &self,
        anchor: &AnchorKey,
    ) -> Result<Vec<BindingRecord>, RuntimeError> {
        self.memory.bindings_on(anchor).await
    }

    pub async fn reaffirm(
        &self,
        reference: &Ref,
        bound_version: Version,
    ) -> Result<(), RuntimeError> {
        reaffirm(&self.log, &self.memory, reference, bound_version).await
    }
}

async fn reaffirm(
    log: &AnchorLog,
    memory: &MemoryLens,
    reference: &Ref,
    bound_version: Version,
) -> Result<(), RuntimeError> {
    let record = memory
        .binding_of(reference)
        .await?
        .ok_or_else(|| RuntimeError::NotBound {
            reference: reference.clone(),
        })?;
    memory
        .bind(
            log,
            &record.binding,
            &bound_version,
            Source::Adjudicated,
            Utc::now(),
        )
        .await
}

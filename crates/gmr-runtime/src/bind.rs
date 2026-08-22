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

    pub async fn revoke(
        &self,
        reference: &Ref,
        source: Source,
    ) -> Result<Vec<AnchorKey>, RuntimeError> {
        let asserted = self.memory.binding_of(reference).await?;
        if asserted.is_empty() {
            return Err(RuntimeError::NotBound {
                reference: reference.clone(),
            });
        }
        let when = Utc::now();
        let mut cleared = Vec::new();
        for anchor in crate::memory::anchors_of(&asserted) {
            let tags: Vec<gmr_store::Tag> = asserted
                .iter()
                .filter(|r| r.binding.anchors.contains(&anchor))
                .map(|r| gmr_store::Tag {
                    binding: r.seq,
                    anchor: anchor.clone(),
                })
                .collect();
            self.memory
                .revoke(&gmr_store::Revocation {
                    reference: reference.clone(),
                    at: anchor.clone(),
                    tags,
                    source,
                    when,
                })
                .await?;
            cleared.push(anchor);
        }
        Ok(cleared)
    }

    pub async fn revoke_on(
        &self,
        reference: &Ref,
        anchors: &[AnchorKey],
        source: Source,
    ) -> Result<(), RuntimeError> {
        let asserted = self.memory.binding_of(reference).await?;
        let when = Utc::now();
        for anchor in anchors {
            let tags: Vec<gmr_store::Tag> = asserted
                .iter()
                .filter(|r| r.binding.anchors.contains(anchor))
                .map(|r| gmr_store::Tag {
                    binding: r.seq,
                    anchor: anchor.clone(),
                })
                .collect();
            if tags.is_empty() {
                continue;
            }
            self.memory
                .revoke(&gmr_store::Revocation {
                    reference: reference.clone(),
                    at: anchor.clone(),
                    tags,
                    source,
                    when,
                })
                .await?;
        }
        Ok(())
    }

    pub async fn bindings_on(
        &self,
        anchor: &AnchorKey,
    ) -> Result<Vec<BindingRecord>, RuntimeError> {
        self.memory.bindings_on(&self.log, anchor).await
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
    let asserted = memory.binding_of(reference).await?;
    if asserted.is_empty() {
        return Err(RuntimeError::NotBound {
            reference: reference.clone(),
        });
    }
    let binding = Binding {
        reference: reference.clone(),
        anchors: crate::memory::anchors_of(&asserted),
    };
    memory
        .bind(
            log,
            &binding,
            &bound_version,
            Source::Adjudicated,
            Utc::now(),
        )
        .await
}

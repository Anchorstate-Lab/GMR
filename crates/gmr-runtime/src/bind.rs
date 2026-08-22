use chrono::Utc;
use gmr_core::{AnchorKey, Binding, Ref, Source, Version, fold};
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
        bound_version: Option<Version>,
        source: Source,
    ) -> Result<Landed, RuntimeError> {
        let mut landed = Landed::default();
        for named in anchors {
            let living = self.living(&named).await?;
            if living != named {
                landed.moved.push((named, living.clone()));
            }
            if !landed.anchors.contains(&living) {
                landed.anchors.push(living);
            }
        }
        self.memory
            .bind(
                &self.log,
                &Binding {
                    reference,
                    anchors: landed.anchors.clone(),
                },
                bound_version.as_ref(),
                source,
                Utc::now(),
            )
            .await?;
        Ok(landed)
    }

    pub async fn living(&self, key: &AnchorKey) -> Result<AnchorKey, RuntimeError> {
        let mut at = key.clone();
        let mut seen = std::collections::BTreeSet::from([at.clone()]);
        for _ in 0..crate::memory::GENERATIONS {
            let Some(state) = fold(&self.log.entries(&at, 0).await?) else {
                break;
            };
            if !state.closed {
                break;
            }
            let Some(heir) = self.heir_of(&at).await? else {
                break;
            };
            if !seen.insert(heir.clone()) {
                break;
            }
            at = heir;
        }
        Ok(at)
    }

    async fn heir_of(&self, key: &AnchorKey) -> Result<Option<AnchorKey>, RuntimeError> {
        for candidate in self.anchors().await? {
            let superseded = fold(&self.log.entries(&candidate, 0).await?)
                .and_then(|s| s.anchor.supersedes.map(|x| x.key));
            if superseded.as_ref() == Some(key) {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
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
        bound_version: Option<Version>,
    ) -> Result<(), RuntimeError> {
        reaffirm(&self.log, &self.memory, reference, bound_version).await
    }
}

async fn reaffirm(
    log: &AnchorLog,
    memory: &MemoryLens,
    reference: &Ref,
    bound_version: Option<Version>,
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
            bound_version.as_ref(),
            Source::Adjudicated,
            Utc::now(),
        )
        .await
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Landed {
    pub anchors: Vec<AnchorKey>,
    pub moved: Vec<(AnchorKey, AnchorKey)>,
}

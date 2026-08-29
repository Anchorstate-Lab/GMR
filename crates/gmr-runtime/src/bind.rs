use chrono::Utc;
use gmr_core::{AnchorKey, Binding, Claim, FactAddress, Source, Version};
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;

impl Runtime {
    pub async fn bind(
        &self,
        claim: Claim,
        anchors: Vec<AnchorKey>,
        bound_version: Option<Version>,
        saw: Option<FactAddress>,
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
        let bound = self.memory.binding_of(&claim).await?;
        if bound.says(&landed.anchors, bound_version.as_ref(), saw.as_ref(), source) {
            return Ok(landed);
        }
        landed.recorded = true;
        self.memory
            .bind(
                &self.log,
                &Binding {
                    claim,
                    anchors: landed.anchors.clone(),
                },
                bound_version.as_ref(),
                saw.as_ref(),
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
            let Some(state) = self.log.state(&at).await? else {
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
            let superseded = self
                .log
                .state(&candidate)
                .await?
                .and_then(|s| s.anchor.supersedes.map(|x| x.key));
            if superseded.as_ref() == Some(key) {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }

    pub async fn revoke(
        &self,
        claim: &Claim,
        source: Source,
    ) -> Result<Vec<AnchorKey>, RuntimeError> {
        let bound = self.memory.binding_of(claim).await?;
        if bound.is_empty() {
            return Err(RuntimeError::NotBound {
                claim: claim.clone(),
            });
        }
        let when = Utc::now();
        let mut cleared = Vec::new();
        for anchor in bound.anchors() {
            self.memory
                .revoke(&gmr_store::Revocation {
                    claim: claim.clone(),
                    at: anchor.clone(),
                    tags: bound.tags_on(anchor),
                    source,
                    when,
                })
                .await?;
            cleared.push(anchor.clone());
        }
        Ok(cleared)
    }

    pub async fn revoke_on(
        &self,
        claim: &Claim,
        anchors: &[AnchorKey],
        source: Source,
    ) -> Result<(), RuntimeError> {
        let bound = self.memory.binding_of(claim).await?;
        let when = Utc::now();
        for anchor in anchors {
            let tags = bound.tags_on(anchor);
            if tags.is_empty() {
                continue;
            }
            self.memory
                .revoke(&gmr_store::Revocation {
                    claim: claim.clone(),
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
    ) -> Result<Vec<crate::memory::Bound>, RuntimeError> {
        self.memory.bindings_on(&self.log, anchor).await
    }

    pub async fn reaffirm(
        &self,
        claim: &Claim,
        bound_version: Option<Version>,
    ) -> Result<(), RuntimeError> {
        reaffirm(&self.log, &self.memory, claim, bound_version).await
    }
}

async fn reaffirm(
    log: &AnchorLog,
    memory: &MemoryLens,
    claim: &Claim,
    bound_version: Option<Version>,
) -> Result<(), RuntimeError> {
    let bound = memory.binding_of(claim).await?;
    if bound.is_empty() {
        return Err(RuntimeError::NotBound {
            claim: claim.clone(),
        });
    }
    let saw = bound.saw().cloned();
    let binding = Binding {
        claim: claim.clone(),
        anchors: bound.anchors().to_vec(),
    };
    memory
        .bind(
            log,
            &binding,
            bound_version.as_ref(),
            saw.as_ref(),
            Source::Adjudicated,
            Utc::now(),
        )
        .await
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Landed {
    pub anchors: Vec<AnchorKey>,
    pub moved: Vec<(AnchorKey, AnchorKey)>,
    pub recorded: bool,
}

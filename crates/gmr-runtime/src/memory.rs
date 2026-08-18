use std::sync::Arc;

use gmr_core::{AnchorKey, Binding, ContentHash, Link, LinkKind, Ref, Version, fold};
use gmr_store::{BindingRecord, BindingStore, LinkStore, Sealer};
use serde::Serialize;

use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::read::{Before, Grounding, MemoryView};
use gmr_content::{ContentError, ContentProvider};
use gmr_probe::Budget;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderWarning {
    pub provider: String,
    pub message: String,
}

pub struct MemoryLens {
    bindings: Arc<dyn BindingStore>,
    sealer: Arc<dyn Sealer>,
    links: Arc<dyn LinkStore>,
    providers: Vec<Arc<dyn ContentProvider>>,
    provider_warnings: Vec<ProviderWarning>,
}

impl MemoryLens {
    pub(crate) fn new(
        bindings: Arc<dyn BindingStore>,
        sealer: Arc<dyn Sealer>,
        links: Arc<dyn LinkStore>,
        providers: Vec<Arc<dyn ContentProvider>>,
        provider_warnings: Vec<ProviderWarning>,
    ) -> Self {
        Self {
            bindings,
            sealer,
            links,
            providers,
            provider_warnings,
        }
    }

    pub fn provider_warnings(&self) -> &[ProviderWarning] {
        &self.provider_warnings
    }

    pub async fn bind(
        &self,
        log: &AnchorLog,
        binding: &Binding,
        bound_version: &Version,
    ) -> Result<(), RuntimeError> {
        let bound_at_seq = match binding.anchors.as_slice() {
            [only] => {
                let entries = log.entries(only, 0).await?;
                fold(&entries).map(|s| s.head)
            }
            _ => None,
        };
        Ok(self
            .bindings
            .bind(binding, bound_version, bound_at_seq)
            .await?)
    }

    pub async fn bindings_on(
        &self,
        anchor: &AnchorKey,
    ) -> Result<Vec<BindingRecord>, RuntimeError> {
        Ok(self.bindings.bindings_on(anchor).await?)
    }

    pub async fn binding_of(&self, reference: &Ref) -> Result<Option<BindingRecord>, RuntimeError> {
        Ok(self.bindings.binding_of(reference).await?)
    }

    pub async fn all(&self) -> Result<Vec<BindingRecord>, RuntimeError> {
        Ok(self.bindings.all().await?)
    }

    pub async fn seal(&self, bytes: &[u8]) -> Result<ContentHash, RuntimeError> {
        Ok(self.sealer.seal(bytes).await?)
    }

    pub async fn sealed(&self, addr: &ContentHash) -> Result<Option<Vec<u8>>, RuntimeError> {
        Ok(self.sealer.sealed(addr).await?)
    }

    pub async fn link(&self, from: &Ref, to: &Ref, kind: LinkKind) -> Result<(), RuntimeError> {
        Ok(self.links.link(from, to, kind).await?)
    }

    pub async fn links_of(&self, reference: &Ref) -> Result<Vec<Link>, RuntimeError> {
        Ok(self.links.links_of(reference).await?)
    }

    fn provider_for(&self, reference: &Ref) -> Option<&Arc<dyn ContentProvider>> {
        self.providers
            .iter()
            .find(|p| p.provider() == &reference.provider)
    }

    pub async fn current_version(
        &self,
        reference: &Ref,
        budget: &Budget,
    ) -> Result<Option<Version>, RuntimeError> {
        let Some(provider) = self.provider_for(reference) else {
            return Err(RuntimeError::NoProvider {
                provider: reference.provider.clone(),
            });
        };
        Ok(provider
            .fetch(&reference.external_id, budget)
            .await?
            .map(|f| f.version))
    }

    pub(crate) async fn fetch_memory(
        &self,
        record: BindingRecord,
        budget: &Budget,
    ) -> Result<MemoryView, RuntimeError> {
        let BindingRecord {
            binding,
            bound_version,
            bound_at_seq,
        } = record;
        Ok(MemoryView {
            links: self.links.links_of(&binding.reference).await?,
            grounded: !binding.anchors.is_empty(),
            grounding: self
                .ground(&binding.reference, &bound_version, budget)
                .await,
            reference: binding.reference,
            bound_version,
            bound_at_seq,
            stale: None,
        })
    }

    async fn ground(&self, reference: &Ref, bound_version: &Version, budget: &Budget) -> Grounding {
        let Some(provider) = self.provider_for(reference) else {
            return Grounding::NoProvider {
                provider: reference.provider.clone(),
            };
        };
        if budget.remaining().is_none() {
            let e = spent();
            return Grounding::Unreachable {
                code: e.code,
                why: e.message,
            };
        }
        let fetched = match provider.fetch(&reference.external_id, budget).await {
            Err(e) => {
                return Grounding::Unreachable {
                    code: e.code,
                    why: e.message,
                };
            }
            Ok(None) => return Grounding::Gone,
            Ok(Some(fetched)) => fetched,
        };
        if &fetched.version == bound_version {
            return Grounding::Current {
                version: fetched.version,
                content: fetched.bytes,
            };
        }
        let before = match provider.history() {
            None => Before::NoHistory,
            Some(history) => {
                match history
                    .fetch_at(&reference.external_id, bound_version, budget)
                    .await
                {
                    Err(e) => Before::Unreachable {
                        code: e.code,
                        why: e.message,
                    },
                    Ok(None) => Before::NotRetained,
                    Ok(Some(content)) => Before::Retrieved { content },
                }
            }
        };
        Grounding::Rewritten {
            version: fetched.version,
            content: fetched.bytes,
            before,
        }
    }

    pub(crate) async fn carry_linked(
        &self,
        memories: &mut Vec<MemoryView>,
        total: &Budget,
        call: std::time::Duration,
    ) -> Result<(), RuntimeError> {
        let linked: Vec<Ref> = memories
            .iter()
            .flat_map(|m| m.links.iter().map(|l| l.to.clone()))
            .collect();

        for reference in linked {
            if memories.iter().any(|m| m.reference == reference) {
                continue;
            }
            let Some(binding) = self.bindings.binding_of(&reference).await? else {
                continue;
            };
            memories.push(self.fetch_memory(binding, &total.narrowed(call)).await?);
        }
        Ok(())
    }
}

fn spent() -> ContentError {
    ContentError::spent(
        "this read had a total budget for reaching content stores and it ran out before \
         this record's turn; nothing was asked, so nothing is known about it",
    )
}

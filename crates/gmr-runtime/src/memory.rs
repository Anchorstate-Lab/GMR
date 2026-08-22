use std::sync::Arc;

use gmr_core::{AnchorKey, Binding, ContentHash, Link, LinkKind, Ref, Source, Version, fold};
use gmr_store::{Asserted, BindingRecord, BindingStore, LinkStore, Revocation, Sealer};
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
        bound_version: Option<&Version>,
        source: Source,
        at: chrono::DateTime<chrono::Utc>,
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
            .bind(&Asserted {
                binding: binding.clone(),
                bound_version: bound_version.cloned(),
                bound_at_seq,
                source,
                at,
            })
            .await?)
    }

    pub async fn bindings_on(
        &self,
        log: &AnchorLog,
        anchor: &AnchorKey,
    ) -> Result<Vec<BindingRecord>, RuntimeError> {
        let chain = chain_from(log, anchor).await?;
        Ok(self.bindings.bindings_on(&chain).await?)
    }

    pub async fn binding_of(&self, reference: &Ref) -> Result<Vec<BindingRecord>, RuntimeError> {
        Ok(self.bindings.binding_of(reference).await?)
    }

    pub async fn revoke(&self, revocation: &Revocation) -> Result<(), RuntimeError> {
        Ok(self.bindings.revoke(revocation).await?)
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
        asserted: Vec<BindingRecord>,
        budget: &Budget,
    ) -> Result<MemoryView, RuntimeError> {
        let baseline = asserted
            .iter()
            .max_by_key(|r| r.seq)
            .expect("a view is only assembled from at least one assertion");
        let reference = baseline.binding.reference.clone();
        let bound_version = baseline.bound_version.clone();
        let bound_at_seq = baseline.bound_at_seq;
        let baseline_at = asserted
            .iter()
            .filter(|r| r.bound_version.is_some())
            .map(|r| r.seq)
            .max();
        let asserted_at = asserted.iter().filter_map(|r| r.asserted_at).min();
        let sources: std::collections::BTreeSet<Source> =
            asserted.iter().map(|r| r.source).collect();
        let anchors: std::collections::BTreeSet<AnchorKey> = asserted
            .iter()
            .flat_map(|r| r.binding.anchors.iter().cloned())
            .collect();

        Ok(MemoryView {
            links: self.links.links_of(&reference).await?,
            grounded: !anchors.is_empty(),
            grounding: self
                .ground(&reference, bound_version.as_ref(), budget)
                .await,
            reference,
            bound_version,
            bound_at_seq,
            baseline_at,
            sources,
            asserted_at,
            stale: None,
        })
    }

    async fn ground(
        &self,
        reference: &Ref,
        bound_version: Option<&Version>,
        budget: &Budget,
    ) -> Grounding {
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
        let Some(bound_version) = bound_version else {
            return Grounding::Unverified {
                version: fetched.version,
                content: fetched.bytes,
            };
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
            let asserted = self.bindings.binding_of(&reference).await?;
            if asserted.is_empty() {
                continue;
            }
            memories.push(self.fetch_memory(asserted, &total.narrowed(call)).await?);
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

pub(crate) const GENERATIONS: usize = 64;

pub(crate) async fn chain_from(
    log: &AnchorLog,
    from: &AnchorKey,
) -> Result<Vec<AnchorKey>, RuntimeError> {
    let mut out = vec![from.clone()];
    let mut seen = std::collections::BTreeSet::from([from.clone()]);
    let mut at = from.clone();

    for _ in 0..GENERATIONS {
        let Some(state) = fold(&log.entries(&at, 0).await?) else {
            break;
        };
        let Some(older) = state.anchor.supersedes.as_ref().map(|s| s.key.clone()) else {
            break;
        };
        if !seen.insert(older.clone()) {
            break;
        }
        out.push(older.clone());
        at = older;
    }
    Ok(out)
}

pub fn by_reference(records: Vec<BindingRecord>) -> Vec<Vec<BindingRecord>> {
    let mut out: Vec<Vec<BindingRecord>> = Vec::new();
    for record in records {
        match out
            .iter_mut()
            .find(|group| group[0].binding.reference == record.binding.reference)
        {
            Some(group) => group.push(record),
            None => out.push(vec![record]),
        }
    }
    out
}

pub fn anchors_of(records: &[BindingRecord]) -> Vec<AnchorKey> {
    records
        .iter()
        .flat_map(|r| r.binding.anchors.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

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
        let bound_at_seq = Some(log.head().await?);
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
    ) -> Result<Vec<Bound>, RuntimeError> {
        let chain = chain_from(log, anchor).await?;
        Ok(by_reference(self.bindings.bindings_on(&chain).await?))
    }

    pub async fn binding_of(&self, reference: &Ref) -> Result<Bound, RuntimeError> {
        Ok(Bound::fold(self.bindings.binding_of(reference).await?))
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
        bound: Bound,
        budget: &Budget,
    ) -> Result<MemoryView, RuntimeError> {
        let standing = bound
            .standing()
            .expect("a view is only assembled from at least one assertion");
        let baseline = bound.baseline().unwrap_or(standing);
        let reference = standing.binding.reference.clone();
        let bound_version = baseline.bound_version.clone();
        let bound_at_seq = baseline.bound_at_seq;
        let baseline_at = baseline.bound_version.as_ref().map(|_| baseline.seq);
        let asserted_at = bound.first_asserted();
        let sources = bound.sources();

        Ok(MemoryView {
            links: self.links.links_of(&reference).await?,
            grounded: !bound.anchors().is_empty(),
            grounding: self
                .ground(&reference, bound_version.as_ref(), budget)
                .await,
            reference,
            bound_version,
            bound_at_seq,
            baseline_at,
            sources,
            asserted_at,
            warrant: None,
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
            let bound = self.binding_of(&reference).await?;
            if bound.is_empty() {
                continue;
            }
            memories.push(self.fetch_memory(bound, &total.narrowed(call)).await?);
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

pub fn by_reference(records: Vec<BindingRecord>) -> Vec<Bound> {
    let mut out: std::collections::BTreeMap<Ref, Vec<BindingRecord>> = Default::default();
    for record in records {
        out.entry(record.binding.reference.clone())
            .or_default()
            .push(record);
    }
    out.into_values().map(Bound::fold).collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bound {
    asserted: Vec<BindingRecord>,
    anchors: Vec<AnchorKey>,
}

impl Bound {
    pub fn fold(asserted: Vec<BindingRecord>) -> Self {
        let anchors = asserted
            .iter()
            .flat_map(|r| r.binding.anchors.iter().cloned())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        Self { asserted, anchors }
    }

    pub fn is_empty(&self) -> bool {
        self.asserted.is_empty()
    }

    pub fn anchors(&self) -> &[AnchorKey] {
        &self.anchors
    }

    pub fn assertions(&self) -> &[BindingRecord] {
        &self.asserted
    }

    pub fn standing(&self) -> Option<&BindingRecord> {
        self.asserted.iter().max_by_key(|r| r.seq)
    }

    pub fn baseline(&self) -> Option<&BindingRecord> {
        self.asserted
            .iter()
            .filter(|r| r.bound_version.is_some())
            .max_by_key(|r| r.seq)
    }

    pub fn bound_version(&self) -> Option<&Version> {
        self.baseline().and_then(|r| r.bound_version.as_ref())
    }

    pub fn sources(&self) -> std::collections::BTreeSet<Source> {
        self.asserted.iter().map(|r| r.source).collect()
    }

    pub fn first_asserted(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.asserted.iter().filter_map(|r| r.asserted_at).min()
    }

    pub fn tags_on(&self, anchor: &AnchorKey) -> Vec<gmr_store::Tag> {
        self.asserted
            .iter()
            .filter(|r| r.binding.anchors.contains(anchor))
            .map(|r| gmr_store::Tag {
                binding: r.seq,
                anchor: anchor.clone(),
            })
            .collect()
    }

    pub fn says(&self, anchors: &[AnchorKey], version: Option<&Version>, source: Source) -> bool {
        !self.asserted.is_empty()
            && anchors.iter().all(|a| self.anchors.contains(a))
            && self.sources().contains(&source)
            && self.bound_version() == version
    }
}

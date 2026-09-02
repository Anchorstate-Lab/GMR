use std::sync::Arc;

use gmr_core::{
    AnchorKey, Binding, Claim, ContentHash, FactAddress, LinkKind, Ref, Source, Version,
};
use gmr_store::{
    Asserted, BindingRecord, BindingStore, LinkRecord, LinkRevocation, LinkStore, Revocation,
    Sealer,
};
use serde::Serialize;

use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::read::{Before, Grounding, MemoryView};
use gmr_budget::Budget;
use gmr_content::{ContentError, ContentProvider};

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
        saw: &std::collections::BTreeSet<FactAddress>,
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
                saw: saw.clone(),
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
        Ok(by_claim(self.bindings.bindings_on(&chain).await?))
    }

    pub async fn binding_of(&self, claim: &Claim) -> Result<Bound, RuntimeError> {
        Ok(Bound::fold(self.bindings.binding_of(claim).await?))
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

    pub async fn link(
        &self,
        from: &Ref,
        to: &Ref,
        kind: LinkKind,
        source: Source,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RuntimeError> {
        Ok(self.links.link(from, to, kind, source, at).await?)
    }

    pub async fn unlink(&self, revocation: &LinkRevocation) -> Result<u64, RuntimeError> {
        Ok(self.links.unlink(revocation).await?)
    }

    pub async fn links_of(&self, reference: &Ref) -> Result<Vec<LinkRecord>, RuntimeError> {
        Ok(self.links.links_of(reference).await?)
    }

    pub async fn all_links(&self) -> Result<Vec<(Ref, LinkRecord)>, RuntimeError> {
        Ok(self.links.all().await?)
    }

    pub async fn links_to(&self, reference: &Ref) -> Result<Vec<(Ref, LinkRecord)>, RuntimeError> {
        Ok(self.links.links_to(reference).await?)
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
        held: Held,
        budget: &Budget,
        lean: bool,
    ) -> Result<MemoryView, RuntimeError> {
        let Held { reference, bound } = held;
        let baseline = bound
            .dating()
            .expect("a view is only assembled from at least one assertion");
        let bound_version = baseline.bound_version.clone();
        let bound_at_seq = baseline.bound_at_seq;
        let baseline_at = baseline.bound_version.as_ref().map(|_| baseline.seq);
        let asserted_at = bound.first_asserted();
        let sources = bound.sources();
        let origin = bound.origin().cloned();

        Ok(MemoryView {
            origin,
            links: self
                .links
                .links_of(&reference)
                .await?
                .into_iter()
                .map(linked)
                .collect(),
            grounded: !bound.anchors().is_empty(),
            grounding: self
                .grounding_of(&reference, bound_version.as_ref(), budget, lean)
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

    pub(crate) async fn grounding_of(
        &self,
        reference: &Ref,
        bound_version: Option<&Version>,
        budget: &Budget,
        lean: bool,
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
        let kept = |bytes: Vec<u8>| (!lean).then_some(bytes);
        let Some(bound_version) = bound_version else {
            return Grounding::Unverified {
                version: fetched.version,
                content: kept(fetched.bytes),
            };
        };
        if &fetched.version == bound_version {
            return Grounding::Current {
                version: fetched.version,
                content: kept(fetched.bytes),
            };
        }
        let before = match (lean, provider.history()) {
            (true, _) => Before::NotAsked,
            (false, None) => Before::NoHistory,
            (false, Some(history)) => {
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
            content: kept(fetched.bytes),
            before,
        }
    }

    pub(crate) async fn carry_linked(
        &self,
        memories: &mut Vec<MemoryView>,
        total: &Budget,
        call: std::time::Duration,
        lean: bool,
    ) -> Result<(), RuntimeError> {
        let linked: Vec<Ref> = memories
            .iter()
            .flat_map(|m| m.links.iter().map(|l| l.to.clone()))
            .collect();

        for reference in linked {
            if memories.iter().any(|m| m.reference == reference) {
                continue;
            }
            let Some(held) = self.binding_of(&Claim::Stored(reference)).await?.held() else {
                continue;
            };
            let mut carried = self.fetch_memory(held, &total.narrowed(call), lean).await?;
            carried.grounded = false;
            memories.push(carried);
        }
        Ok(())
    }
}

fn linked(record: LinkRecord) -> crate::read::Linked {
    crate::read::Linked {
        to: record.to,
        kind: record.kind,
        source: record.source,
        at: record.at,
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
        let Some(state) = log.state(&at).await? else {
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

pub fn by_claim(records: Vec<BindingRecord>) -> Vec<Bound> {
    let mut out: std::collections::BTreeMap<String, Vec<BindingRecord>> = Default::default();
    for record in records {
        out.entry(record.binding.claim.identity().to_string())
            .or_default()
            .push(record);
    }
    out.into_values().map(Bound::fold).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    reference: Ref,
    bound: Bound,
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

    pub fn dating(&self) -> Option<&BindingRecord> {
        self.baseline().or_else(|| self.standing())
    }

    pub fn bound_version(&self) -> Option<&Version> {
        self.baseline().and_then(|r| r.bound_version.as_ref())
    }

    pub fn claim(&self) -> Option<&Claim> {
        self.standing().map(|r| &r.binding.claim)
    }

    pub fn stored(&self) -> Option<&Ref> {
        self.standing().and_then(|r| r.binding.stored())
    }

    pub fn held(self) -> Option<Held> {
        let reference = self.stored()?.clone();
        Some(Held {
            reference,
            bound: self,
        })
    }

    pub fn saw(&self) -> &std::collections::BTreeSet<FactAddress> {
        static NOTHING: std::sync::LazyLock<std::collections::BTreeSet<FactAddress>> =
            std::sync::LazyLock::new(Default::default);
        self.standing().map_or(&NOTHING, |r| &r.saw)
    }

    pub fn depends(&self) -> Option<&gmr_core::Expr> {
        self.standing().and_then(|r| r.binding.depends.as_ref())
    }

    pub fn origin(&self) -> Option<&gmr_core::SaidId> {
        self.standing().and_then(|r| r.binding.origin.as_ref())
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

    pub fn says(
        &self,
        asking: &Binding,
        version: Option<&Version>,
        saw: &std::collections::BTreeSet<FactAddress>,
        source: Source,
    ) -> bool {
        !self.asserted.is_empty()
            && asking.anchors.iter().all(|a| self.anchors.contains(a))
            && self.sources().contains(&source)
            && self.bound_version() == version
            && self.saw() == saw
            && self.depends() == asking.depends.as_ref()
            && self.dating().is_some_and(|r| r.bound_at_seq.is_some())
    }
}

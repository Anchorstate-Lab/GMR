use std::sync::Arc;

use gmr_core::{AnchorKey, Binding, ContentHash, Link, LinkKind, Ref, Version, fold};
use gmr_store::{BindingRecord, BindingStore, LinkStore, Sealer};
use serde::Serialize;

use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::read::MemoryView;
use gmr_content::ContentProvider;

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

    pub async fn current_version(&self, reference: &Ref) -> Result<Option<Version>, RuntimeError> {
        let Some(provider) = self.provider_for(reference) else {
            return Ok(None);
        };
        Ok(provider
            .fetch(&reference.external_id)
            .await?
            .map(|f| f.version))
    }

    pub(crate) async fn fetch_memory(
        &self,
        record: BindingRecord,
    ) -> Result<MemoryView, RuntimeError> {
        let BindingRecord {
            binding,
            bound_version,
            bound_at_seq,
        } = record;
        let links = self.links.links_of(&binding.reference).await?;
        let mut view = MemoryView {
            reference: binding.reference.clone(),
            bound_version: bound_version.clone(),
            current_version: None,
            rewritten: false,
            content: None,
            content_at_bind: None,
            retrievable: None,
            grounded: !binding.anchors.is_empty(),
            unavailable: None,
            links,
            bound_at_seq,
            stale: None,
        };

        let Some(provider) = self.provider_for(&binding.reference) else {
            view.unavailable = Some(format!(
                "no provider recognises a `{}` reference",
                binding.reference.provider
            ));
            return Ok(view);
        };

        match provider.fetch(&binding.reference.external_id).await {
            Err(e) => view.unavailable = Some(e.message),
            Ok(None) => view.unavailable = Some("the provider says this record is gone".to_owned()),
            Ok(Some(fetched)) => {
                view.rewritten = fetched.version != bound_version;
                view.current_version = Some(fetched.version);
                match String::from_utf8(fetched.bytes) {
                    Ok(text) => view.content = Some(text),
                    Err(_) => view.unavailable = Some("the record is not UTF-8 text".to_owned()),
                }

                if view.rewritten {
                    match provider
                        .fetch_at(&binding.reference.external_id, &bound_version)
                        .await
                    {
                        Ok(Some(bytes)) => {
                            view.retrievable = Some(true);
                            view.content_at_bind = String::from_utf8(bytes).ok();
                        }
                        Ok(None) => view.retrievable = Some(false),
                        Err(e) => view.unavailable = Some(e.message),
                    }
                } else {
                    view.retrievable = Some(true);
                }
            }
        }
        Ok(view)
    }

    pub(crate) async fn carry_linked(
        &self,
        memories: &mut Vec<MemoryView>,
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
            memories.push(self.fetch_memory(binding).await?);
        }
        Ok(())
    }
}

use std::sync::Arc;

use gmr_store::{BindingStore, Journal, LinkStore, Queue, Sealer, Settings};

use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::{MemoryLens, ProviderWarning};
use crate::observer::Observer;
use crate::policy::Policy;
use crate::scheduler::Scheduler;
use gmr_content::ContentProvider;

pub struct Runtime {
    pub(crate) log: AnchorLog,
    pub(crate) observer: Observer,
    pub(crate) memory: MemoryLens,
    pub(crate) scheduler: Scheduler,
}

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::default()
    }

    pub fn log(&self) -> &AnchorLog {
        &self.log
    }

    pub fn memory(&self) -> &MemoryLens {
        &self.memory
    }

    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    pub fn policy(&self) -> &Policy {
        self.scheduler.policy()
    }

    pub async fn anchors(&self) -> Result<Vec<gmr_core::AnchorKey>, RuntimeError> {
        self.log.anchors().await
    }
}

#[derive(Default)]
pub struct RuntimeBuilder {
    transports: Vec<Arc<dyn gmr_probe::Transport>>,
    journal: Option<Arc<dyn Journal>>,
    bindings: Option<Arc<dyn BindingStore>>,
    sealer: Option<Arc<dyn Sealer>>,
    links: Option<Arc<dyn LinkStore>>,
    providers: Vec<Arc<dyn ContentProvider>>,
    provider_warnings: Vec<ProviderWarning>,
    queue: Option<Arc<dyn Queue>>,
    settings: Option<Arc<dyn Settings>>,
    policy: Option<Policy>,
}

impl RuntimeBuilder {
    pub fn transport(mut self, t: Arc<dyn gmr_probe::Transport>) -> Self {
        self.transports.push(t);
        self
    }

    pub fn journal(mut self, j: Arc<dyn Journal>) -> Self {
        self.journal = Some(j);
        self
    }

    pub fn bindings(mut self, b: Arc<dyn BindingStore>) -> Self {
        self.bindings = Some(b);
        self
    }

    pub fn sealer(mut self, s: Arc<dyn Sealer>) -> Self {
        self.sealer = Some(s);
        self
    }

    pub fn links(mut self, l: Arc<dyn LinkStore>) -> Self {
        self.links = Some(l);
        self
    }

    pub fn provider(mut self, p: Arc<dyn ContentProvider>) -> Self {
        self.providers.push(p);
        self
    }

    pub fn provider_warning(
        mut self,
        provider: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.provider_warnings.push(ProviderWarning {
            provider: provider.into(),
            message: message.into(),
        });
        self
    }

    pub fn queue(mut self, q: Arc<dyn Queue>) -> Self {
        self.queue = Some(q);
        self
    }

    pub fn settings(mut self, s: Arc<dyn Settings>) -> Self {
        self.settings = Some(s);
        self
    }

    pub fn policy(mut self, p: Policy) -> Self {
        self.policy = Some(p);
        self
    }

    pub fn build(self) -> Runtime {
        Runtime {
            log: AnchorLog::new(self.journal.expect("a Journal is not optional")),
            observer: Observer::new(self.transports),
            memory: MemoryLens::new(
                self.bindings.expect("a BindingStore is not optional"),
                self.sealer.expect("a Sealer is not optional"),
                self.links.expect("a LinkStore is not optional"),
                self.providers,
                self.provider_warnings,
            ),
            scheduler: Scheduler::new(
                self.queue,
                self.settings.expect("a Settings store is not optional"),
                self.policy.unwrap_or_default(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmr_store::testkit::{MemoryBindings, MemoryJournal, MemoryQueue};

    #[test]
    fn a_provider_warning_reaches_the_built_runtime() {
        let bindings = Arc::new(MemoryBindings::default());
        let rt = Runtime::builder()
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(bindings.clone())
            .sealer(bindings.clone())
            .links(bindings)
            .settings(Arc::new(MemoryQueue::default()))
            .provider_warning("claude-code", "$HOME is not set")
            .build();

        let warnings = rt.memory().provider_warnings();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].provider, "claude-code");
        assert_eq!(warnings[0].message, "$HOME is not set");
    }

    #[test]
    fn no_warnings_by_default() {
        let bindings = Arc::new(MemoryBindings::default());
        let rt = Runtime::builder()
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(bindings.clone())
            .sealer(bindings.clone())
            .links(bindings)
            .settings(Arc::new(MemoryQueue::default()))
            .build();

        assert!(rt.memory().provider_warnings().is_empty());
    }
}

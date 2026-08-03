use std::sync::Arc;

use gmr_store::{BindingStore, Journal, LinkStore, Queue, Sealer};

use crate::content::ContentProvider;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;
use crate::observer::Observer;
use crate::policy::Policy;
use crate::scheduler::Scheduler;

/// A facade over four services, split by what each may touch. Verb modules
/// take only the services they need, so a new verb inherits no capability by
/// default.
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
    queue: Option<Arc<dyn Queue>>,
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

    pub fn queue(mut self, q: Arc<dyn Queue>) -> Self {
        self.queue = Some(q);
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
            ),
            scheduler: Scheduler::new(self.queue, self.policy.unwrap_or_default()),
        }
    }
}

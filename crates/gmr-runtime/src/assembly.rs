use std::sync::Arc;

use gmr_store::{BindingStore, Journal, Queue};

use crate::content::ContentProvider;
use crate::error::RuntimeError;
use crate::policy::Policy;

pub struct Runtime {
    pub(crate) transports: Vec<Arc<dyn gmr_probe::Transport>>,
    pub(crate) journal: Arc<dyn Journal>,
    pub(crate) bindings: Arc<dyn BindingStore>,
    pub(crate) providers: Vec<Arc<dyn ContentProvider>>,
    pub(crate) queue: Option<Arc<dyn Queue>>,
    pub(crate) policy: Policy,
}

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::default()
    }

    pub fn journal(&self) -> &dyn Journal {
        self.journal.as_ref()
    }

    pub fn bindings(&self) -> &dyn BindingStore {
        self.bindings.as_ref()
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    pub(crate) fn has_lease(&self) -> bool {
        self.queue.is_some()
    }

    pub async fn anchors(&self) -> Result<Vec<gmr_core::AnchorKey>, RuntimeError> {
        Ok(self.journal.anchors().await?)
    }
}

#[derive(Default)]
pub struct RuntimeBuilder {
    transports: Vec<Arc<dyn gmr_probe::Transport>>,
    journal: Option<Arc<dyn Journal>>,
    bindings: Option<Arc<dyn BindingStore>>,
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
            transports: self.transports,
            journal: self.journal.expect("Journal 非有不可"),
            bindings: self.bindings.expect("BindingStore 非有不可"),
            providers: self.providers,
            queue: self.queue,
            policy: self.policy.unwrap_or_default(),
        }
    }
}

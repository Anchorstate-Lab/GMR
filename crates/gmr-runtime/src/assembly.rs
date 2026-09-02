use std::sync::Arc;

use gmr_store::{Journal, Ledger, LinkStore, Queue, Sealer, Settings, Sightings, Usage};
use gmr_store::BindingStore;

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
    pub(crate) usage: Option<Arc<dyn Usage>>,
    pub(crate) ledger: Option<Arc<dyn Ledger>>,
    pub(crate) session: String,
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

    pub fn content_budget(&self) -> gmr_budget::Budget {
        self.scheduler.policy().content_budget()
    }

    pub async fn anchors(&self) -> Result<Vec<gmr_core::AnchorKey>, RuntimeError> {
        self.log.anchors().await
    }

    pub async fn used(&self, claim: &gmr_core::Claim) -> Result<(), RuntimeError> {
        let Some(usage) = &self.usage else {
            return Ok(());
        };
        Ok(usage.used(claim, chrono::Utc::now()).await?)
    }

    pub async fn usage_of(&self, claim: &gmr_core::Claim) -> Result<gmr_store::Used, RuntimeError> {
        let Some(usage) = &self.usage else {
            return Ok(gmr_store::Used::default());
        };
        Ok(usage.usage_of(claim).await?)
    }

    pub async fn all_usage(
        &self,
    ) -> Result<Vec<(gmr_core::Claim, gmr_store::Used)>, RuntimeError> {
        let Some(usage) = &self.usage else {
            return Ok(Vec::new());
        };
        Ok(usage.all_usage().await?)
    }

    pub fn session(&self) -> &str {
        &self.session
    }

    pub async fn spent(&self, verb: &str, bytes: u64) -> Result<(), RuntimeError> {
        let Some(ledger) = &self.ledger else {
            return Ok(());
        };
        Ok(ledger
            .spent(&self.session, verb, bytes, chrono::Utc::now())
            .await?)
    }

    pub async fn spending(&self) -> Result<Vec<gmr_store::Spending>, RuntimeError> {
        let Some(ledger) = &self.ledger else {
            return Ok(Vec::new());
        };
        Ok(ledger.spending().await?)
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
    sightings: Option<Arc<dyn Sightings>>,
    usage: Option<Arc<dyn Usage>>,
    ledger: Option<Arc<dyn Ledger>>,
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

    pub fn sightings(mut self, s: Arc<dyn Sightings>) -> Self {
        self.sightings = Some(s);
        self
    }

    pub fn usage(mut self, u: Arc<dyn Usage>) -> Self {
        self.usage = Some(u);
        self
    }

    pub fn ledger(mut self, l: Arc<dyn Ledger>) -> Self {
        self.ledger = Some(l);
        self
    }

    pub fn policy(mut self, p: Policy) -> Self {
        self.policy = Some(p);
        self
    }

    pub fn build(self) -> Runtime {
        match self.try_build() {
            Ok(runtime) => runtime,
            Err(wrong) => panic!("{wrong}"),
        }
    }

    pub fn try_build(self) -> Result<Runtime, AssemblyError> {
        let mut named: Vec<&gmr_core::ProviderId> = Vec::new();
        for provider in &self.providers {
            let id = provider.provider();
            if named.contains(&id) {
                return Err(AssemblyError::TwoUnderOneName { id: id.clone() });
            }
            named.push(id);
        }
        Ok(Runtime {
            log: AnchorLog::new(self.journal.ok_or(AssemblyError::Missing {
                part: Part::Journal,
            })?),
            observer: Observer::new(self.transports),
            memory: MemoryLens::new(
                self.bindings.ok_or(AssemblyError::Missing {
                    part: Part::Bindings,
                })?,
                self.sealer
                    .ok_or(AssemblyError::Missing { part: Part::Sealer })?,
                self.links
                    .ok_or(AssemblyError::Missing { part: Part::Links })?,
                self.providers,
                self.provider_warnings,
            ),
            scheduler: Scheduler::new(
                self.queue,
                self.settings.ok_or(AssemblyError::Missing {
                    part: Part::Settings,
                })?,
                self.sightings.ok_or(AssemblyError::Missing {
                    part: Part::Sightings,
                })?,
                self.policy.unwrap_or_default(),
            ),
            usage: self.usage,
            ledger: self.ledger,
            session: format!(
                "{}-{}",
                chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
                std::process::id()
            ),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Journal,
    Bindings,
    Sealer,
    Links,
    Settings,
    Sightings,
}

impl std::fmt::Display for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Journal => "a Journal",
            Self::Bindings => "a BindingStore",
            Self::Sealer => "a Sealer",
            Self::Links => "a LinkStore",
            Self::Settings => "a Settings store",
            Self::Sightings => {
                "a Sightings store: a look that found nothing new is recorded there \
                 instead of in the log"
            }
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AssemblyError {
    #[error("{part} is not optional")]
    Missing { part: Part },

    #[error(
        "two providers are registered as `{id}`, and lookup takes the first match — \
         so every reference through `{id}` would silently resolve against one of them \
         and never the other. Give each instance its own name at assembly time"
    )]
    TwoUnderOneName { id: gmr_core::ProviderId },
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmr_core::{ExternalId, ProviderId};
    use gmr_store::testkit::{MemoryBindings, MemoryJournal, MemoryQueue};

    struct Named(ProviderId);

    #[async_trait::async_trait]
    impl ContentProvider for Named {
        fn provider(&self) -> &ProviderId {
            &self.0
        }

        async fn fetch(
            &self,
            _id: &ExternalId,
            _budget: &gmr_budget::Budget,
        ) -> Result<Option<gmr_content::Fetched>, gmr_content::ContentError> {
            Ok(None)
        }
    }

    fn assembled(providers: [&str; 2]) -> Runtime {
        let bindings = Arc::new(MemoryBindings::default());
        let mut builder = Runtime::builder()
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(bindings.clone())
            .sealer(bindings.clone())
            .links(bindings)
            .settings(Arc::new(MemoryQueue::default()))
            .sightings(Arc::new(MemoryQueue::default()));
        for id in providers {
            builder = builder.provider(Arc::new(Named(ProviderId::new(id))));
        }
        builder.build()
    }

    #[test]
    #[should_panic(expected = "two providers are registered as `mem0`")]
    fn two_providers_under_one_name_is_refused_at_assembly() {
        assembled(["mem0", "mem0"]);
    }

    #[test]
    fn a_runtime_built_from_a_configuration_is_told_which_part_is_missing() {
        let missing = Runtime::builder()
            .journal(Arc::new(MemoryJournal::default()))
            .try_build();

        let Err(AssemblyError::Missing { part }) = missing else {
            panic!("a builder handed only a journal cannot produce a Runtime")
        };
        assert_eq!(
            part,
            Part::Bindings,
            "a service assembling itself from a file somebody wrote has to say which line \
             was wrong. `build` still panics, which stays right for a binary that wrote its \
             own assembly -- and it calls this, so there is one definition of complete"
        );
    }

    #[test]
    fn the_duplicate_name_check_reports_rather_than_aborts_on_that_path() {
        let bindings = Arc::new(MemoryBindings::default());
        let built = Runtime::builder()
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(bindings.clone())
            .sealer(bindings.clone())
            .links(bindings)
            .settings(Arc::new(MemoryQueue::default()))
            .sightings(Arc::new(MemoryQueue::default()))
            .provider(Arc::new(Named(ProviderId::new("mem0"))))
            .provider(Arc::new(Named(ProviderId::new("mem0"))))
            .try_build();
        assert!(matches!(built, Err(AssemblyError::TwoUnderOneName { .. })));
    }

    #[test]
    fn two_instances_of_one_backend_under_different_names_are_fine() {
        assembled(["mem0-work", "mem0-personal"]);
    }

    #[test]
    fn a_provider_warning_reaches_the_built_runtime() {
        let bindings = Arc::new(MemoryBindings::default());
        let rt = Runtime::builder()
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(bindings.clone())
            .sealer(bindings.clone())
            .links(bindings)
            .settings(Arc::new(MemoryQueue::default()))
            .sightings(Arc::new(MemoryQueue::default()))
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
            .sightings(Arc::new(MemoryQueue::default()))
            .build();

        assert!(rt.memory().provider_warnings().is_empty());
    }
}

use chrono::Utc;
use gmr_budget::Budget;
use gmr_core::{
    Anchor, AnchorKey, Entry, FailureCode, Observation, ReasonClass, Recorded, State, Versions,
    fold, should_still,
};
use gmr_expr::EVALUATOR_VERSION;
use gmr_store::{Disposition, Fence};

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::observer::Observer;
use crate::scheduler::Scheduler;
use crate::translate::{Transitioned, transition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    Transitioned {
        from: State,
        to: State,
    },
    Unchanged {
        state: State,
    },
    Still,
    Attempt {
        reason: ReasonClass,
        code: FailureCode,
        message: String,
        attempts: u32,
    },
    Closed,
}

impl Runtime {
    pub fn instrument(
        &self,
        probe: &gmr_core::ProbeRef,
    ) -> Result<gmr_core::Derivation, gmr_probe::ProbeError> {
        self.observer.resolve(probe)
    }

    pub async fn sample(
        &self,
        probe: &gmr_core::ProbeRef,
        position: &serde_json::Value,
    ) -> Result<gmr_core::Outcome, gmr_probe::ProbeError> {
        let budget = self.scheduler.policy().budget();
        self.observer.sample(probe, position, &budget).await
    }

    pub async fn observe(&self, key: &AnchorKey) -> Result<Observed, RuntimeError> {
        self.observe_within(key, &crate::read::Instructions::default())
            .await
    }

    pub async fn observe_within(
        &self,
        key: &AnchorKey,
        how: &crate::read::Instructions,
    ) -> Result<Observed, RuntimeError> {
        let policy = self.scheduler.policy();
        let budget = match how.budget {
            Some(span) => policy.budget().narrowed(span),
            None => policy.budget(),
        };
        observe(&self.log, &self.observer, &self.scheduler, key, &budget).await
    }
}

async fn observe(
    log: &AnchorLog,
    observer: &Observer,
    scheduler: &Scheduler,
    key: &AnchorKey,
    budget: &Budget,
) -> Result<Observed, RuntimeError> {
    if !scheduler.leases_configured() {
        return observe_with(log, observer, scheduler, key, Fence::Unleased, budget).await;
    }

    let now = chrono::Utc::now();
    let lease = chrono::Duration::seconds(scheduler.policy().lease_secs as i64);
    let Some(ticket) = scheduler.lease(key, now, lease).await? else {
        return Err(RuntimeError::Leased { key: key.clone() });
    };

    let seen = observe_with(log, observer, scheduler, key, ticket.fence, budget).await;
    let after = match &seen {
        Ok(Observed::Closed) => Disposition::Retire,
        _ => Disposition::Reschedule {
            after_secs: scheduler.cadence_for(key).await?,
        },
    };
    scheduler.settle(&ticket, after, Utc::now()).await?;
    seen
}

pub(crate) async fn observe_with(
    log: &AnchorLog,
    observer: &Observer,
    scheduler: &Scheduler,
    key: &AnchorKey,
    fence: Fence,
    budget: &Budget,
) -> Result<Observed, RuntimeError> {
    let entries = log.entries(key, 0).await?;
    let s = fold(&entries).ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

    if s.closed {
        return Ok(Observed::Closed);
    }

    let at = Utc::now();

    let derivation = match observer.resolve(&s.anchor.probe) {
        Ok(d) => d,
        Err(e) => {
            return record_attempt(log, key, e.code.into(), e.message, fence, s.attempts() + 1)
                .await;
        }
    };

    let run = scheduler.settings_for(key).await?;
    let mine = match run.budget_ms {
        Some(ms) => budget.narrowed(std::time::Duration::from_millis(ms)),
        None => budget.clone(),
    };

    let outcome = match observer.invoke(&s.anchor, s.position(), &mine).await {
        Ok(o) => o,
        Err(e) => {
            return record_attempt(log, key, e.code.into(), e.message, fence, s.attempts() + 1)
                .await;
        }
    };

    let observation = match observe_into(&s.anchor, outcome, derivation, run.facts) {
        Ok(o) => o,
        Err(e @ RuntimeError::Undigested { .. }) => {
            return record_attempt(
                log,
                key,
                FailureCode::Unusable,
                e.to_string(),
                fence,
                s.attempts() + 1,
            )
            .await;
        }
        Err(e) => return Err(e),
    };
    let entered_at = s.entered_at.unwrap_or(at);

    let next = match transition(&s.anchor, &observation, &s.state, at, entered_at) {
        Transitioned::To(next) => next,
        Transitioned::Unchanged => s.state.clone(),
        Transitioned::Unevaluable(code, message) => {
            return record_attempt(log, key, code, message, fence, s.attempts() + 1).await;
        }
    };

    let still_ref = if run.retains_full() {
        None
    } else {
        s.latest
            .as_ref()
            .filter(|last| {
                should_still(
                    &s.state,
                    &last.fact_address,
                    &next,
                    &observation.fact_address,
                )
            })
            .and(s.latest_seq)
    };

    match still_ref {
        Some(ref_entry) if s.attempts() > 0 => {
            log.append(
                key,
                &Entry::Still {
                    ref_entry,
                    at,
                    versions: observation.versions.clone(),
                },
                fence,
            )
            .await?;
        }
        Some(_) => {}
        None => {
            log.append(
                key,
                &Entry::Transition {
                    observation,
                    state: next.clone(),
                    at,
                },
                fence,
            )
            .await?;
        }
    }
    scheduler.sighted(key, at).await?;

    Ok(match still_ref {
        Some(_) => Observed::Still,
        None if s.state == next => Observed::Unchanged { state: next },
        None => Observed::Transitioned {
            from: s.state,
            to: next,
        },
    })
}

async fn record_attempt(
    log: &AnchorLog,
    key: &AnchorKey,
    code: FailureCode,
    message: String,
    fence: Fence,
    attempts: u32,
) -> Result<Observed, RuntimeError> {
    let reason = code.reason();
    log.append(
        key,
        &Entry::Attempt {
            reason,
            code: Some(code),
            message: message.clone(),
            at: Utc::now(),
        },
        fence,
    )
    .await?;
    Ok(Observed::Attempt {
        reason,
        code,
        message,
        attempts,
    })
}

pub(crate) fn observe_into(
    anchor: &Anchor,
    outcome: gmr_core::Outcome,
    derivation: gmr_core::Derivation,
    recorded: Recorded,
) -> Result<Observation, RuntimeError> {
    if matches!(recorded, Recorded::Digests) && !outcome.digested() {
        return Err(RuntimeError::Undigested {
            key: anchor.key.clone(),
        });
    }
    let fact_address = outcome.address(&derivation.version)?;
    Ok(Observation {
        outcome,
        fact_address,
        versions: Versions {
            declaration: anchor.probe.declaration_hash()?,
            derivation,
            evaluator: EVALUATOR_VERSION.to_owned(),
        },
    })
}

use chrono::DateTime;
use chrono::Utc;
use futures_util::{StreamExt, TryStreamExt};
use gmr_budget::Budget;
use gmr_core::{
    Anchor, AnchorKey, AnchorState, Entry, FailureCode, Observation, ReasonClass, Recorded,
    RunSettings, State, Versions, should_still,
};
use gmr_expr::EVALUATOR_VERSION;
use gmr_store::{Disposition, Expected, Fence};

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::{AnchorLog, Stood};
use crate::observer::Observer;
use crate::read::AnchorView;
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
    Contended,
}

#[derive(Debug, Clone)]
pub struct Looked {
    pub before: AnchorView,
    pub observed: Observed,
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
        Ok(self.looking(key, how).await?.0)
    }

    pub async fn look(&self, key: &AnchorKey) -> Result<Looked, RuntimeError> {
        self.look_within(key, &crate::read::Instructions::default())
            .await
    }

    pub async fn look_within(
        &self,
        key: &AnchorKey,
        how: &crate::read::Instructions,
    ) -> Result<Looked, RuntimeError> {
        let looks = self.scheduler.seen(key).await?;
        let (observed, stood) = self.looking(key, how).await?;
        let (before, _) = crate::read::viewed(stood.anchor, key, &looks, stood.logged);
        Ok(Looked { before, observed })
    }

    pub async fn look_all(&self, keys: &[AnchorKey]) -> Result<Vec<Looked>, RuntimeError> {
        self.look_all_within(keys, &crate::read::Instructions::default())
            .await
    }

    pub async fn look_all_within(
        &self,
        keys: &[AnchorKey],
        how: &crate::read::Instructions,
    ) -> Result<Vec<Looked>, RuntimeError> {
        futures_util::stream::iter(keys)
            .map(|key| self.look_within(key, how))
            .buffered(self.scheduler.policy().observe_at_once.max(1))
            .try_collect()
            .await
    }

    async fn looking(
        &self,
        key: &AnchorKey,
        how: &crate::read::Instructions,
    ) -> Result<(Observed, Stood), RuntimeError> {
        let policy = self.scheduler.policy();
        let budget = match how.budget {
            Some(span) => policy.budget().narrowed(span),
            None => policy.budget(),
        };
        observe(&self.log, &self.observer, &self.scheduler, key, &budget).await
    }
}

pub(crate) async fn observe(
    log: &AnchorLog,
    observer: &Observer,
    scheduler: &Scheduler,
    key: &AnchorKey,
    budget: &Budget,
) -> Result<(Observed, Stood), RuntimeError> {
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
        Ok((Observed::Closed, _)) => Disposition::Retire,
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
) -> Result<(Observed, Stood), RuntimeError> {
    let stood = log
        .stood(key)
        .await?
        .ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

    let observed = looked_at(log, observer, scheduler, key, fence, budget, &stood.anchor).await?;
    Ok((observed, stood))
}

#[allow(clippy::too_many_arguments)]
async fn looked_at(
    log: &AnchorLog,
    observer: &Observer,
    scheduler: &Scheduler,
    key: &AnchorKey,
    fence: Fence,
    budget: &Budget,
    s: &AnchorState,
) -> Result<Observed, RuntimeError> {
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

    recorded(log, scheduler, key, fence, at, &observation, run, s.clone()).await
}

const REPLAYS: u32 = 3;

#[allow(clippy::too_many_arguments)]
async fn recorded(
    log: &AnchorLog,
    scheduler: &Scheduler,
    key: &AnchorKey,
    fence: Fence,
    at: DateTime<Utc>,
    observation: &Observation,
    run: RunSettings,
    stood: AnchorState,
) -> Result<Observed, RuntimeError> {
    let mut s = stood;

    for _ in 0..REPLAYS {
        if s.closed {
            return Ok(Observed::Closed);
        }

        let entered_at = s.entered_at.unwrap_or(at);
        let next = match transition(&s.anchor, observation, &s.state, at, entered_at) {
            Transitioned::To(next) => next,
            Transitioned::Unchanged => s.state.clone(),
            Transitioned::Unevaluable(code, message) => {
                return record_attempt(log, key, code, message, fence, s.attempts() + 1).await;
            }
        };

        let still_ref = match run.retains_full() {
            true => None,
            false => s
                .latest
                .as_ref()
                .filter(|last| {
                    should_still(
                        &s.state,
                        &last.fact_address,
                        &next,
                        &observation.fact_address,
                    )
                })
                .and(s.latest_seq),
        };

        let entry = match still_ref {
            Some(ref_entry) if s.attempts() > 0 => Some(Entry::Still {
                ref_entry,
                at,
                versions: observation.versions.clone(),
            }),
            Some(_) => None,
            None => Some(Entry::Transition {
                observation: observation.clone(),
                state: next.clone(),
                at,
            }),
        };

        if let Some(entry) = &entry
            && let Err(e) = log.append(key, entry, fence, Expected::Head(s.head)).await
        {
            if !e.head_moved() {
                return Err(e);
            }
            let Some(again) = log.stood(key).await? else {
                return Err(RuntimeError::NoSuchAnchor { key: key.clone() });
            };
            s = again.anchor;
            continue;
        }

        scheduler.sighted(key, at).await?;

        return Ok(match still_ref {
            Some(_) => Observed::Still,
            None if s.state == next => Observed::Unchanged { state: next },
            None => Observed::Transitioned {
                from: s.state.clone(),
                to: next,
            },
        });
    }

    Ok(Observed::Contended)
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
        Expected::Any,
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

use chrono::{Duration, Utc};
use gmr_core::{AnchorKey, ReasonClass, RunSettings};
use gmr_store::Disposition;
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::observe::{Observed, observe_with};
use crate::observer::Observer;
use crate::scheduler::Scheduler;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Passed {
    pub observed: usize,
    pub moved: Vec<AnchorKey>,
    pub unseen: usize,
    pub retired: usize,
    pub skipped: usize,
}

impl Runtime {
    pub async fn ensure_scheduled(&self, key: &AnchorKey) -> Result<bool, RuntimeError> {
        ensure_scheduled(&self.log, &self.scheduler, key).await
    }

    pub async fn requeue(&self, key: &AnchorKey) -> Result<bool, RuntimeError> {
        self.scheduler.requeue_now(key, Utc::now()).await
    }

    pub async fn settings_for(&self, key: &AnchorKey) -> Result<RunSettings, RuntimeError> {
        self.scheduler.settings_for(key).await
    }

    pub async fn set_settings(
        &self,
        key: &AnchorKey,
        settings: &RunSettings,
    ) -> Result<(), RuntimeError> {
        self.scheduler.set_settings(key, settings).await
    }

    pub async fn pass(&self) -> Result<Passed, RuntimeError> {
        pass(&self.log, &self.observer, &self.scheduler).await
    }
}

async fn ensure_scheduled(
    log: &AnchorLog,
    scheduler: &Scheduler,
    key: &AnchorKey,
) -> Result<bool, RuntimeError> {
    match log.state(key).await? {
        Some(state) if !state.closed => scheduler.ensure_enqueued(key, Utc::now()).await,
        _ => Ok(false),
    }
}

async fn pass(
    log: &AnchorLog,
    observer: &Observer,
    scheduler: &Scheduler,
) -> Result<Passed, RuntimeError> {
    let now = Utc::now();
    let tickets = scheduler
        .due(
            now,
            Duration::seconds(scheduler.policy().lease_secs as i64),
            scheduler.policy().batch,
        )
        .await?;

    let budget = scheduler.policy().budget();
    let mut out = Passed::default();
    for ticket in tickets {
        if budget.remaining().is_none() {
            out.skipped += 1;
            scheduler
                .settle(
                    &ticket,
                    Disposition::Reschedule { after_secs: 0 },
                    Utc::now(),
                )
                .await?;
            continue;
        }

        let (observed, stood) = observe_with(
            log,
            observer,
            scheduler,
            &ticket.anchor,
            ticket.fence,
            &budget,
        )
        .await?;
        out.observed += 1;

        let disposition = match &observed {
            Observed::Closed => {
                out.retired += 1;
                Disposition::Retire
            }
            Observed::Attempt {
                reason, attempts, ..
            } => {
                out.unseen += 1;
                Disposition::Backoff {
                    after_secs: match reason {
                        ReasonClass::Unevaluable => scheduler.policy().backoff_cap_secs as i64,
                        _ => scheduler.policy().backoff_secs(*attempts),
                    },
                }
            }
            other => {
                if matches!(other, Observed::Transitioned { .. }) {
                    out.moved.push(ticket.anchor.clone());
                }
                let sealed = matches!(other, Observed::Transitioned { to, .. }
                    if stood.anchor.anchor.is_terminal(to));
                if sealed {
                    out.retired += 1;
                    Disposition::Retire
                } else {
                    Disposition::Reschedule {
                        after_secs: scheduler.cadence_for(&ticket.anchor).await?,
                    }
                }
            }
        };
        scheduler.settle(&ticket, disposition, Utc::now()).await?;
    }
    Ok(out)
}

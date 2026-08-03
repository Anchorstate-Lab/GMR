use chrono::{Duration, Utc};
use gmr_core::{AnchorKey, ReasonClass, fold};
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
    pub moved: usize,
    pub unseen: usize,
    pub retired: usize,
}

impl Runtime {
    pub async fn schedule(&self, key: &AnchorKey) -> Result<bool, RuntimeError> {
        schedule(&self.log, &self.scheduler, key).await
    }

    pub async fn pass(&self) -> Result<Passed, RuntimeError> {
        pass(&self.log, &self.observer, &self.scheduler).await
    }
}

async fn schedule(
    log: &AnchorLog,
    scheduler: &Scheduler,
    key: &AnchorKey,
) -> Result<bool, RuntimeError> {
    let entries = log.entries(key, 0).await?;
    match fold(&entries) {
        Some(state) if !state.closed => scheduler.enqueue(key, Utc::now()).await,
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

    let mut out = Passed::default();
    for ticket in tickets {
        let observed = observe_with(log, observer, &ticket.anchor, ticket.fence).await?;
        out.observed += 1;

        let disposition = match &observed {
            Observed::Closed => {
                out.retired += 1;
                Disposition::Retire
            }
            // Our failures and the world's do not share a backoff: a blown
            // expression blows up just the same sooner or later, and rushing to
            // retry only spams the log.
            Observed::Attempt { reason, .. } => {
                out.unseen += 1;
                let attempts = fold(&log.entries(&ticket.anchor, 0).await?)
                    .map(|s| s.attempts)
                    .unwrap_or(1);
                Disposition::Backoff {
                    after_secs: match reason {
                        ReasonClass::Unevaluable => scheduler.policy().backoff_cap_secs as i64,
                        _ => scheduler.policy().backoff_secs(attempts),
                    },
                }
            }
            other => {
                if matches!(other, Observed::Transitioned { from, to } if from != to) {
                    out.moved += 1;
                }
                let sealed = matches!(other, Observed::Transitioned { to, .. }
                    if fold(&log.entries(&ticket.anchor, 0).await?)
                        .is_some_and(|s| s.anchor.is_terminal(to)));
                if sealed {
                    out.retired += 1;
                    Disposition::Retire
                } else {
                    Disposition::Reschedule {
                        after_secs: cadence_of(log, scheduler, &ticket.anchor).await?,
                    }
                }
            }
        };
        scheduler.settle(&ticket, disposition, Utc::now()).await?;
    }
    Ok(out)
}

pub(crate) async fn cadence_of(
    log: &AnchorLog,
    scheduler: &Scheduler,
    key: &AnchorKey,
) -> Result<i64, RuntimeError> {
    let entries = log.entries(key, 0).await?;
    Ok(fold(&entries)
        .and_then(|s| s.anchor.cadence_secs)
        .unwrap_or(scheduler.policy().cadence_secs) as i64)
}

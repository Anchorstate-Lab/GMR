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
            // retry only spams the log. `attempts` came back with the Observed
            // itself — no need to re-fold the journal just to learn what it
            // was just told.
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
                if matches!(other, Observed::Transitioned { from, to } if from != to) {
                    out.moved += 1;
                }
                // The anchor's own declaration (terminal set, cadence) cannot
                // have changed mid-observation, so one fold serves both checks
                // instead of two separate re-reads of the journal.
                let anchor = fold(&log.entries(&ticket.anchor, 0).await?).map(|s| s.anchor);
                let sealed = matches!(other, Observed::Transitioned { to, .. }
                    if anchor.as_ref().is_some_and(|a| a.is_terminal(to)));
                if sealed {
                    out.retired += 1;
                    Disposition::Retire
                } else {
                    let after_secs = anchor
                        .as_ref()
                        .and_then(|a| a.cadence_secs)
                        .unwrap_or(scheduler.policy().cadence_secs)
                        as i64;
                    Disposition::Reschedule { after_secs }
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

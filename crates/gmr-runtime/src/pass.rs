use chrono::{Duration, Utc};
use gmr_core::{ReasonClass, fold};
use gmr_store::Disposition;
use serde::Serialize;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::observe::Observed;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Passed {
    pub observed: usize,
    pub moved: usize,
    pub unseen: usize,
    pub retired: usize,
}

impl Runtime {
    pub async fn schedule(&self, key: &gmr_core::AnchorKey) -> Result<bool, RuntimeError> {
        let Some(queue) = self.queue.as_ref() else {
            return Ok(false);
        };
        let entries = self.journal.entries(key, 0).await?;
        match fold(&entries) {
            Some(state) if !state.closed => {
                queue.enqueue(key, Utc::now()).await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub async fn pass(&self) -> Result<Passed, RuntimeError> {
        let Some(queue) = self.queue.clone() else {
            return Err(RuntimeError::NoQueue);
        };
        let now = Utc::now();
        let tickets = queue
            .due(
                now,
                Duration::seconds(self.policy.lease_secs as i64),
                self.policy.batch,
            )
            .await?;

        let mut out = Passed::default();
        for ticket in tickets {
            let observed = self.observe_with(&ticket.anchor, ticket.fence).await?;
            out.observed += 1;

            let disposition = match &observed {
                Observed::Closed => {
                    out.retired += 1;
                    Disposition::Retire
                }
                // 我们的失败和世界的失败不共用退避：表达式炸了，早一点晚
                // 一点重试都一样炸，急着重试只是在刷日志。
                Observed::Attempt { reason, .. } => {
                    out.unseen += 1;
                    let attempts = fold(&self.journal.entries(&ticket.anchor, 0).await?)
                        .map(|s| s.attempts)
                        .unwrap_or(1);
                    Disposition::Backoff {
                        after_secs: match reason {
                            ReasonClass::Unevaluable => self.policy.backoff_cap_secs as i64,
                            _ => self.policy.backoff_secs(attempts),
                        },
                    }
                }
                other => {
                    if matches!(other, Observed::Transitioned { from, to } if from != to) {
                        out.moved += 1;
                    }
                    let sealed = matches!(other, Observed::Transitioned { to, .. }
                        if fold(&self.journal.entries(&ticket.anchor, 0).await?)
                            .is_some_and(|s| s.anchor.is_terminal(to)));
                    if sealed {
                        out.retired += 1;
                        Disposition::Retire
                    } else {
                        Disposition::Reschedule {
                            after_secs: self.cadence_of(&ticket.anchor).await?,
                        }
                    }
                }
            };
            queue.settle(&ticket, disposition, Utc::now()).await?;
        }
        Ok(out)
    }

    async fn cadence_of(&self, key: &gmr_core::AnchorKey) -> Result<i64, RuntimeError> {
        let entries = self.journal.entries(key, 0).await?;
        Ok(fold(&entries)
            .and_then(|s| s.anchor.cadence_secs)
            .unwrap_or(self.policy.cadence_secs) as i64)
    }
}

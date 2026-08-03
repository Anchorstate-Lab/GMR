use chrono::Utc;
use gmr_core::{
    Anchor, AnchorKey, Entry, Observation, ReasonClass, State, Versions, fold, should_still,
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
    Still,
    Attempt {
        reason: ReasonClass,
        message: String,
    },
    Closed,
}

impl Runtime {
    /// Observe one anchor by hand.
    ///
    /// In a queued deployment this takes the anchor's lease before writing —
    /// writing past the token is precisely the second writer the lease prevents.
    /// If someone else holds it, let them write.
    pub async fn observe(&self, key: &AnchorKey) -> Result<Observed, RuntimeError> {
        observe(&self.log, &self.observer, &self.scheduler, key).await
    }
}

async fn observe(
    log: &AnchorLog,
    observer: &Observer,
    scheduler: &Scheduler,
    key: &AnchorKey,
) -> Result<Observed, RuntimeError> {
    if !scheduler.has_lease() {
        return observe_with(log, observer, key, Fence::Unleased).await;
    }

    let now = chrono::Utc::now();
    let lease = chrono::Duration::seconds(scheduler.policy().lease_secs as i64);
    let Some(ticket) = scheduler.lease(key, now, lease).await? else {
        return Err(RuntimeError::Leased { key: key.clone() });
    };

    let seen = observe_with(log, observer, key, ticket.fence).await;
    let after = match &seen {
        Ok(Observed::Closed) => Disposition::Retire,
        _ => Disposition::Reschedule {
            after_secs: crate::pass::cadence_of(log, scheduler, key).await?,
        },
    };
    scheduler.settle(&ticket, after, Utc::now()).await?;
    seen
}

pub(crate) async fn observe_with(
    log: &AnchorLog,
    observer: &Observer,
    key: &AnchorKey,
    fence: Fence,
) -> Result<Observed, RuntimeError> {
    let entries = log.entries(key, 0).await?;
    let s = fold(&entries).ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

    if s.closed {
        return Ok(Observed::Closed);
    }

    let at = Utc::now();

    let sighted = match observer.invoke(&s.anchor, s.position()).await {
        Ok(o) => o,
        Err(e) => return record_attempt(log, key, e.reason, e.message, fence).await,
    };

    let observation = observe_into(&s.anchor, sighted);
    let entered_at = s.entered_at.unwrap_or(at);

    let next = match transition(&s.anchor, &observation, &s.state, at, entered_at) {
        Transitioned::To(next) => next,
        Transitioned::Unchanged => s.state.clone(),
        Transitioned::Unevaluable(message) => {
            return record_attempt(log, key, ReasonClass::Unevaluable, message, fence).await;
        }
    };

    let still_ref = if gmr_core::journal::always_full(&s.anchor) {
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

    let entry = match still_ref {
        Some(ref_entry) => Entry::Still {
            ref_entry,
            at,
            versions: observation.versions.clone(),
        },
        None => Entry::Transition {
            observation,
            state: next.clone(),
            at,
        },
    };

    log.append(key, &entry, fence).await?;

    Ok(match still_ref {
        Some(_) => Observed::Still,
        None => Observed::Transitioned {
            from: s.state,
            to: next,
        },
    })
}

async fn record_attempt(
    log: &AnchorLog,
    key: &AnchorKey,
    reason: ReasonClass,
    message: String,
    fence: Fence,
) -> Result<Observed, RuntimeError> {
    log.append(
        key,
        &Entry::Attempt {
            reason,
            message: message.clone(),
            at: Utc::now(),
        },
        fence,
    )
    .await?;
    Ok(Observed::Attempt { reason, message })
}

pub(crate) fn observe_into(anchor: &Anchor, sighted: gmr_probe::Sighted) -> Observation {
    let gmr_probe::Sighted {
        outcome,
        derivation,
    } = sighted;
    // The address is fixed by **the rule that actually derived it**, not by the
    // declaration on the anchor. NotFound is addressed too — absence is an answer.
    let fact_address = outcome.address(&derivation.version);
    Observation {
        outcome,
        fact_address,
        versions: Versions {
            declaration: anchor.probe.declaration_hash(),
            derivation,
            evaluator: EVALUATOR_VERSION.to_owned(),
        },
    }
}

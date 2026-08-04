use std::collections::BTreeSet;

use chrono::Utc;
use gmr_core::{
    Anchor, AnchorKey, Entry, ProbeRef, RunSettings, State, StatusId, Superseded, Transitions, fold,
};
use gmr_store::Fence;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;
use crate::observer::Observer;
use crate::scheduler::Scheduler;
use crate::translate::{Transitioned, bind_warnings, transition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub key: AnchorKey,
    pub state: State,
    pub warnings: Vec<String>,
    pub supersedes: Option<AnchorKey>,
}

/// The rationale is mandatory: superseding is a change of heart about criteria.
pub struct Supersede {
    pub key: AnchorKey,
    pub rationale: Vec<u8>,
}

pub struct OpenRequest {
    pub key: AnchorKey,
    pub probe: ProbeRef,
    pub transitions: Transitions,
    pub terminal: BTreeSet<StatusId>,
    pub initial: Option<State>,
    /// How it is run. Not sealed into the anchor, and changeable afterwards
    /// without a rationale — see [`gmr_core::RunSettings`].
    pub settings: RunSettings,
    pub supersedes: Option<Supersede>,
}

impl Runtime {
    pub async fn open(&self, request: OpenRequest) -> Result<Opened, RuntimeError> {
        open(
            &self.log,
            &self.observer,
            &self.memory,
            &self.scheduler,
            request,
        )
        .await
    }
}

async fn open(
    log: &AnchorLog,
    observer: &Observer,
    memory: &MemoryLens,
    scheduler: &Scheduler,
    request: OpenRequest,
) -> Result<Opened, RuntimeError> {
    let key = request.key.clone();
    if !log.entries(&key, 0).await?.is_empty() {
        return Err(RuntimeError::AlreadyOpen { key });
    }

    let sealed = match request.supersedes {
        None => None,
        Some(s) => Some(seal_supersede(log, memory, s).await?),
    };
    let supersedes = sealed.as_ref().map(|s| s.key.clone());

    let anchor = Anchor {
        key: key.clone(),
        probe: request.probe,
        transitions: request.transitions,
        terminal: request.terminal,
        supersedes: sealed,
    };

    let initial = request.initial.unwrap_or_default();

    // A name nothing provides is a typo; saying so beats opening an anchor that
    // can only ever produce identical failures.
    let derivation = observer
        .resolve(&anchor.probe)
        .map_err(|e| RuntimeError::CannotOpen { message: e.message })?;

    let outcome = observer
        .invoke(&anchor, initial.position())
        .await
        .map_err(|e| RuntimeError::CannotOpen { message: e.message })?;

    let at = Utc::now();
    let observation = crate::observe::observe_into(&anchor, outcome, derivation);
    let mut warnings = bind_warnings(&anchor, &observation);
    warnings.extend(accumulator_warning(scheduler, &anchor));

    // Failing to compute the first state is no reason to refuse: an anchor may
    // precede its target, and then the rules naturally resolve to nothing. A
    // typo and "not grown yet" look identical at this moment; both surface at
    // the first real observation — and there it is loud.
    let state = match transition(&anchor, &observation, &initial, at, at) {
        Transitioned::To(next) => next,
        Transitioned::Unchanged => initial,
        Transitioned::Unevaluable(_, message) => {
            warnings.push(format!("{message}; the initial state is kept as is"));
            initial
        }
    };

    log.append(
        &key,
        &Entry::Open {
            anchor: Box::new(anchor),
            observation,
            state: state.clone(),
            at,
        },
        Fence::Unleased,
    )
    .await?;

    // Same recoverable side branch as enqueueing below: the settings are
    // mutable, so a failure here costs the deployment default until someone
    // sets them again, not the anchor.
    if let Err(e) = scheduler.set_settings(&key, &request.settings).await {
        warnings.push(format!(
            "the anchor opened but its retain/cadence could not be stored ({e}); \
             it runs on the deployment defaults until sync sets them"
        ));
    }

    // Journal and queue are two stores with no shared transaction, so be clear
    // about who decides: **the anchor is that log entry**, and once it lands the
    // anchor exists. Enqueueing is a recoverable side branch; failing it must
    // not misreport "already open" as "failed to open" — that makes the caller
    // retry, hit AlreadyOpen, and still leave the real gap unrepaired.
    if let Err(e) = scheduler.ensure_enqueued(&key, at).await {
        warnings.push(format!(
            "the anchor opened but could not be enqueued ({e}); it will not be \
             observed automatically until the next sync repairs it"
        ));
    }

    Ok(Opened {
        key,
        state,
        warnings,
        supersedes,
    })
}

/// The old one must really have finished — two generations alive at once is a
/// bypass around finishing.
async fn seal_supersede(
    log: &AnchorLog,
    memory: &MemoryLens,
    s: Supersede,
) -> Result<Superseded, RuntimeError> {
    let old = fold(&log.entries(&s.key, 0).await?)
        .ok_or_else(|| RuntimeError::NoSuchAnchor { key: s.key.clone() })?;
    if !old.closed {
        return Err(RuntimeError::NotClosedYet { key: s.key });
    }
    Ok(Superseded {
        key: s.key,
        rationale: memory.seal(&s.rationale).await?,
    })
}

fn accumulator_warning(scheduler: &Scheduler, anchor: &Anchor) -> Option<String> {
    if scheduler.leases_configured() {
        return None;
    }
    let reads_state = anchor
        .transitions
        .iter()
        .any(|r| crate::translate::compile(&r.to).is_ok_and(|n| n.reads_state()));
    reads_state.then(|| {
        "the transition table reads the previous state into the new one, and \
         this deployment has no lease: a repeated observation would over-count. \
         Idempotent forms (carrying a field through unchanged) are unaffected; \
         increments are not. The substrate cannot tell them apart, so it only warns"
            .to_owned()
    })
}

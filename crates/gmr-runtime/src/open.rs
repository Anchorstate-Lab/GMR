use std::collections::BTreeSet;

use chrono::Utc;
use gmr_core::{
    Anchor, AnchorKey, Entry, ProbeRef, RunSettings, State, StatusId, Superseded, Transitions,
};
use gmr_store::{Expected, Fence};
use serde::{Deserialize, Serialize};

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::log::AnchorLog;
use crate::memory::MemoryLens;
use crate::observer::Observer;
use crate::scheduler::Scheduler;
use crate::translate::{Transitioned, bind_warnings, transition};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Opened {
    pub key: AnchorKey,
    pub state: State,
    pub warnings: Vec<String>,
    pub supersedes: Option<AnchorKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Supersede {
    pub key: AnchorKey,
    #[serde(with = "written")]
    pub rationale: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenRequest {
    pub key: AnchorKey,
    pub probe: ProbeRef,
    #[serde(default, deserialize_with = "authored")]
    pub transitions: Transitions,
    #[serde(default)]
    pub terminal: BTreeSet<StatusId>,
    #[serde(default)]
    pub initial: Option<State>,
    #[serde(default)]
    pub settings: RunSettings,
    #[serde(default)]
    pub supersedes: Option<Supersede>,
}

mod written {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        Ok(String::deserialize(d)?.into_bytes())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    when: String,
    to: String,
}

fn authored<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Transitions, D::Error> {
    Ok(Transitions(
        Vec::<Written>::deserialize(d)?
            .into_iter()
            .map(|rule| gmr_core::Rule {
                when: gmr_core::Expr::text(rule.when),
                to: gmr_core::Expr::text(rule.to),
            })
            .collect(),
    ))
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

    let derivation = observer
        .resolve(&anchor.probe)
        .map_err(|e| RuntimeError::CannotOpen { message: e.message })?;

    let outcome = observer
        .invoke(&anchor, initial.position(), &scheduler.policy().budget())
        .await
        .map_err(|e| RuntimeError::CannotOpen { message: e.message })?;

    let at = Utc::now();
    let observation =
        crate::observe::observe_into(&anchor, outcome, derivation, request.settings.facts)?;
    let mut warnings = bind_warnings(&anchor, &observation);
    warnings.extend(accumulator_warning(scheduler, &anchor));

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
        Expected::Head(0),
    )
    .await?;

    scheduler.sighted(&key, at).await?;

    if let Err(e) = scheduler.set_settings(&key, &request.settings).await {
        warnings.push(format!(
            "the anchor opened but its retain/cadence could not be stored ({e}); \
             it runs on the deployment defaults until sync sets them"
        ));
    }

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

async fn seal_supersede(
    log: &AnchorLog,
    memory: &MemoryLens,
    s: Supersede,
) -> Result<Superseded, RuntimeError> {
    let old = log
        .state(&s.key)
        .await?
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

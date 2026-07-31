use chrono::Utc;
use gmr_core::{
    Anchor, AnchorKey, Entry, FactAddress, Observation, Outcome, ReasonClass, State, Versions,
    fold, should_still,
};
use gmr_expr::EVALUATOR_VERSION;
use gmr_store::Fence;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
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
    pub async fn observe(&self, key: &AnchorKey) -> Result<Observed, RuntimeError> {
        self.observe_with(key, Fence::NONE).await
    }

    pub(crate) async fn observe_with(
        &self,
        key: &AnchorKey,
        fence: Fence,
    ) -> Result<Observed, RuntimeError> {
        let entries = self.journal.entries(key, 0).await?;
        let s = fold(&entries).ok_or_else(|| RuntimeError::NoSuchAnchor { key: key.clone() })?;

        if s.closed {
            return Ok(Observed::Closed);
        }

        let at = Utc::now();

        let outcome = match self.invoke(&s.anchor, s.position()).await {
            Ok(o) => o,
            Err(e) => return self.record_attempt(key, e.reason, e.message, fence).await,
        };

        let observation = observe_into(&s.anchor, outcome);
        let entered_at = s.entered_at.unwrap_or(at);

        let next = match transition(&s.anchor, &observation, &s.state, at, entered_at) {
            Transitioned::To(next) => next,
            Transitioned::Unchanged => s.state.clone(),
            Transitioned::Unevaluable(message) => {
                return self
                    .record_attempt(key, ReasonClass::Unevaluable, message, fence)
                    .await;
            }
        };

        let last = last_sighting(&entries);
        let still = !gmr_core::journal::always_full(&s.anchor)
            && last.as_ref().is_some_and(|(_, state, address)| {
                should_still(
                    state,
                    address.as_ref(),
                    &next,
                    observation.fact_address.as_ref(),
                )
            });

        let entry = if still {
            Entry::Still {
                ref_entry: last.expect("still 要求有上一次观测").0,
                at,
                versions: observation.versions.clone(),
            }
        } else {
            Entry::Transition {
                observation,
                state: next.clone(),
                at,
            }
        };

        self.journal.append(key, &entry, fence).await?;

        Ok(if still {
            Observed::Still
        } else {
            Observed::Transitioned {
                from: s.state,
                to: next,
            }
        })
    }

    async fn record_attempt(
        &self,
        key: &AnchorKey,
        reason: ReasonClass,
        message: String,
        fence: Fence,
    ) -> Result<Observed, RuntimeError> {
        self.journal
            .append(
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

    pub(crate) async fn invoke(
        &self,
        anchor: &Anchor,
        position: &serde_json::Value,
    ) -> Result<Outcome, gmr_probe::ProbeError> {
        let transport = self
            .transports
            .iter()
            .find(|t| t.kind() == &anchor.probe.kind)
            .ok_or_else(|| {
                gmr_probe::ProbeError::unreachable(format!(
                    "没有传输认得 `{}` 这种探针",
                    anchor.probe.kind
                ))
            })?;
        transport.invoke(&anchor.probe.declaration, position).await
    }
}

pub(crate) fn observe_into(anchor: &Anchor, outcome: Outcome) -> Observation {
    let versions = Versions {
        probe: anchor.probe.version(),
        evaluator: EVALUATOR_VERSION.to_owned(),
    };
    let fact_address = match &outcome {
        Outcome::Found { facts } => Some(facts.address(&versions.probe)),
        Outcome::NotFound => None,
    };
    Observation {
        outcome,
        fact_address,
        versions,
    }
}

fn last_sighting(entries: &[(u64, Entry)]) -> Option<(u64, State, Option<FactAddress>)> {
    let mut carried: Option<(State, Option<FactAddress>)> = None;
    let mut seq: Option<u64> = None;

    for (n, entry) in entries {
        match entry {
            Entry::Open {
                observation, state, ..
            }
            | Entry::Transition {
                observation, state, ..
            } => {
                carried = Some((state.clone(), observation.fact_address.clone()));
                seq = Some(*n);
            }
            Entry::Still { .. } => seq = Some(*n),
            _ => {}
        }
    }

    match (seq, carried) {
        (Some(seq), Some((state, address))) => Some((seq, state, address)),
        _ => None,
    }
}

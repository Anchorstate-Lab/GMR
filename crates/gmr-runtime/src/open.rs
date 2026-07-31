use std::collections::BTreeSet;

use chrono::Utc;
use gmr_core::{Anchor, AnchorKey, Entry, Probe, Retain, State, StatusId, Transitions};
use gmr_store::Fence;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::translate::{Transitioned, bind_warnings, transition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub key: AnchorKey,
    pub state: State,
    pub warnings: Vec<String>,
}

pub struct OpenRequest {
    pub key: AnchorKey,
    pub probe: Probe,
    pub transitions: Transitions,
    pub terminal: BTreeSet<StatusId>,
    pub initial: Option<State>,
    pub retain: Retain,
    pub cadence_secs: Option<u64>,
}

impl Runtime {
    pub async fn open(&self, request: OpenRequest) -> Result<Opened, RuntimeError> {
        let key = request.key.clone();
        if !self.journal.entries(&key, 0).await?.is_empty() {
            return Err(RuntimeError::AlreadyOpen { key });
        }

        let anchor = Anchor {
            key: key.clone(),
            probe: request.probe,
            transitions: if request.transitions.is_empty() {
                Transitions::watch_everything()
            } else {
                request.transitions
            },
            terminal: request.terminal,
            retain: request.retain,
            cadence_secs: request.cadence_secs,
        };

        let initial = request.initial.unwrap_or_default();

        let outcome = self
            .invoke(&anchor, initial.position())
            .await
            .map_err(|e| RuntimeError::CannotOpen { message: e.message })?;

        let at = Utc::now();
        let observation = crate::observe::observe_into(&anchor, outcome);
        let mut warnings = bind_warnings(&anchor, &observation);
        warnings.extend(self.accumulator_warning(&anchor));

        let state = match transition(&anchor, &observation, &initial, at, at) {
            Transitioned::To(next) => next,
            Transitioned::Unchanged => initial,
            Transitioned::Unevaluable(message) => {
                return Err(RuntimeError::CannotTransition { message });
            }
        };

        self.journal
            .append(
                &key,
                &Entry::Open {
                    anchor: Box::new(anchor),
                    observation,
                    state: state.clone(),
                    at,
                },
                Fence::NONE,
            )
            .await?;

        if let Some(queue) = self.queue.as_ref() {
            queue.enqueue(&key, at).await?;
        }

        Ok(Opened {
            key,
            state,
            warnings,
        })
    }

    fn accumulator_warning(&self, anchor: &Anchor) -> Option<String> {
        if self.has_lease() {
            return None;
        }
        let reads_state = anchor
            .transitions
            .iter()
            .any(|r| crate::translate::compile(&r.to).is_ok_and(|n| n.render().contains("state.")));
        reads_state.then(|| {
            "转换表把上一个状态读进了新状态，而这个部署没有租约：\
             重复观测会让累积量多算。幂等的写法（原样保留某个字段）不受影响，\
             递增之类的会。基底分辨不了两者，所以只提醒"
                .to_owned()
        })
    }
}

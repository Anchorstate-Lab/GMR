use std::collections::BTreeSet;

use chrono::Utc;
use gmr_core::{
    Anchor, AnchorKey, Entry, ProbeRef, Retain, State, StatusId, Superseded, Transitions, fold,
};
use gmr_store::Fence;

use crate::assembly::Runtime;
use crate::error::RuntimeError;
use crate::translate::{Transitioned, bind_warnings, transition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub key: AnchorKey,
    pub state: State,
    pub warnings: Vec<String>,
    pub supersedes: Option<AnchorKey>,
}

/// 理由必填：接替是判据层面的反悔。
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
    pub retain: Retain,
    pub cadence_secs: Option<u64>,
    pub supersedes: Option<Supersede>,
}

impl Runtime {
    pub async fn open(&self, request: OpenRequest) -> Result<Opened, RuntimeError> {
        let key = request.key.clone();
        if !self.journal.entries(&key, 0).await?.is_empty() {
            return Err(RuntimeError::AlreadyOpen { key });
        }

        let sealed = match request.supersedes {
            None => None,
            Some(s) => Some(self.seal_supersede(s).await?),
        };
        let supersedes = sealed.as_ref().map(|s| s.key.clone());

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
            supersedes: sealed,
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

        // 算不出第一个状态不是拒绝的理由：锚可以先于它的目标存在，那时
        // 规则本来就取不到东西。拼错和「还没长出来」在这一刻长得一样，
        // 两者都留到第一次真观测再现形 —— 那时它响。
        let state = match transition(&anchor, &observation, &initial, at, at) {
            Transitioned::To(next) => next,
            Transitioned::Unchanged => initial,
            Transitioned::Unevaluable(message) => {
                warnings.push(format!("{message}；起始状态原样留着"));
                initial
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
            supersedes,
        })
    }

    /// 旧的必须真的终结了 —— 否则两代同时活着，就是绕过终结的旁路。
    async fn seal_supersede(&self, s: Supersede) -> Result<Superseded, RuntimeError> {
        let old = fold(&self.journal.entries(&s.key, 0).await?)
            .ok_or_else(|| RuntimeError::NoSuchAnchor { key: s.key.clone() })?;
        if !old.closed {
            return Err(RuntimeError::NotClosedYet { key: s.key });
        }
        Ok(Superseded {
            key: s.key,
            rationale: self.bindings.seal(&s.rationale).await?,
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

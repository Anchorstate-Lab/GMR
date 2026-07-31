use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::addr::ContentHash;
use crate::anchor::{Anchor, Retain, State, StatusId, Transitions};
use crate::probe::{FactAddress, Facts, Outcome, Probe, ProbeVersion};

pub type Seq = u64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versions {
    pub probe: ProbeVersion,
    pub evaluator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_address: Option<FactAddress>,
    pub versions: Versions,
}

impl Observation {
    pub fn facts(&self) -> Option<&Facts> {
        match &self.outcome {
            Outcome::Found { facts } => Some(facts),
            Outcome::NotFound => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "revise", rename_all = "snake_case")]
pub enum Change {
    Reprobe { probe: Probe },
    Retransition { transitions: Transitions },
    Reterminal { terminal: BTreeSet<StatusId> },
    Restate { state: State },
}

impl Change {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Reprobe { .. } => "reprobe",
            Self::Retransition { .. } => "retransition",
            Self::Reterminal { .. } => "reterminal",
            Self::Restate { .. } => "restate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonClass {
    Unreachable,
    Unusable,
    Unevaluable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "snake_case")]
pub enum Entry {
    Open {
        anchor: Box<Anchor>,
        observation: Observation,
        state: State,
        at: DateTime<Utc>,
    },
    Transition {
        observation: Observation,
        state: State,
        at: DateTime<Utc>,
    },
    Still {
        ref_entry: Seq,
        at: DateTime<Utc>,
        versions: Versions,
    },
    Attempt {
        reason: ReasonClass,
        message: String,
        at: DateTime<Utc>,
    },
    Revise {
        change: Change,
        context: ContentHash,
        rationale: ContentHash,
        at: DateTime<Utc>,
    },
    Close {
        context: ContentHash,
        rationale: ContentHash,
        at: DateTime<Utc>,
    },
}

impl Entry {
    pub fn at(&self) -> DateTime<Utc> {
        match self {
            Self::Open { at, .. }
            | Self::Transition { at, .. }
            | Self::Still { at, .. }
            | Self::Attempt { at, .. }
            | Self::Revise { at, .. }
            | Self::Close { at, .. } => *at,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Open { .. } => "open",
            Self::Transition { .. } => "transition",
            Self::Still { .. } => "still",
            Self::Attempt { .. } => "attempt",
            Self::Revise { .. } => "revise",
            Self::Close { .. } => "close",
        }
    }

    pub fn is_sighting(&self) -> bool {
        matches!(
            self,
            Self::Open { .. } | Self::Transition { .. } | Self::Still { .. }
        )
    }
}

pub fn should_still(
    last_state: &State,
    last_address: Option<&FactAddress>,
    now_state: &State,
    now_address: Option<&FactAddress>,
) -> bool {
    last_state == now_state && last_address == now_address
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorState {
    pub anchor: Anchor,
    pub state: State,
    pub latest: Option<Observation>,
    pub latest_seq: Option<Seq>,
    pub closed: bool,
    pub attempts: u32,
    pub last_sighting: Option<DateTime<Utc>>,
    pub entered_at: Option<DateTime<Utc>>,
    pub head: Seq,
    pub revisions: BTreeMap<String, u32>,
}

impl AnchorState {
    pub fn position(&self) -> &serde_json::Value {
        self.state.position()
    }
}

pub fn fold(entries: &[(Seq, Entry)]) -> Option<AnchorState> {
    let mut acc: Option<AnchorState> = None;

    for (seq, entry) in entries {
        match entry {
            Entry::Open {
                anchor,
                observation,
                state,
                at,
            } => {
                acc = Some(AnchorState {
                    anchor: (**anchor).clone(),
                    state: state.clone(),
                    latest: Some(observation.clone()),
                    latest_seq: Some(*seq),
                    closed: false,
                    attempts: 0,
                    last_sighting: Some(*at),
                    entered_at: Some(*at),
                    head: *seq,
                    revisions: BTreeMap::new(),
                });
            }
            Entry::Transition {
                observation,
                state,
                at,
            } => {
                let Some(s) = acc.as_mut() else { continue };
                if s.state != *state {
                    s.entered_at = Some(*at);
                }
                s.state = state.clone();
                s.latest = Some(observation.clone());
                s.latest_seq = Some(*seq);
                s.attempts = 0;
                s.last_sighting = Some(*at);
                s.head = *seq;
            }
            Entry::Still { at, .. } => {
                let Some(s) = acc.as_mut() else { continue };
                s.attempts = 0;
                s.last_sighting = Some(*at);
                s.head = *seq;
            }
            Entry::Attempt { .. } => {
                let Some(s) = acc.as_mut() else { continue };
                s.attempts += 1;
                s.head = *seq;
            }
            Entry::Revise { change, at, .. } => {
                let Some(s) = acc.as_mut() else { continue };
                apply(s, change, *at);
                s.head = *seq;
            }
            Entry::Close { .. } => {
                let Some(s) = acc.as_mut() else { continue };
                s.closed = true;
                s.head = *seq;
            }
        }
    }

    if let Some(s) = acc.as_mut() {
        s.closed = s.closed || s.anchor.is_terminal(&s.state);
    }
    acc
}

fn apply(s: &mut AnchorState, change: &Change, at: DateTime<Utc>) {
    *s.revisions
        .entry(change.kind_name().to_owned())
        .or_insert(0) += 1;

    match change {
        Change::Reprobe { probe } => s.anchor.probe = probe.clone(),
        Change::Retransition { transitions } => s.anchor.transitions = transitions.clone(),
        Change::Reterminal { terminal } => s.anchor.terminal = terminal.clone(),
        Change::Restate { state } => {
            if s.state != *state {
                s.entered_at = Some(at);
            }
            s.state = state.clone();
        }
    }
}

pub fn always_full(anchor: &Anchor) -> bool {
    matches!(anchor.retain, Retain::Full)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::{AnchorKey, Expr, POSITION, Rule, STATUS};
    use crate::probe::{Kind, Probe};
    use serde_json::json;

    fn versions() -> Versions {
        Versions {
            probe: ProbeVersion::new("a".repeat(64)),
            evaluator: "eval-1".to_owned(),
        }
    }

    fn anchor(terminal: &[&str]) -> Anchor {
        Anchor {
            key: AnchorKey::new("a"),
            probe: Probe::new(Kind::new("shell"), json!({ "run": "x" })),
            transitions: Transitions(vec![Rule {
                when: Expr::text("changed(\"shape\")"),
                to: Expr::text("{ status: \"drifted\" }"),
            }]),
            terminal: terminal.iter().map(|s| StatusId::new(*s)).collect(),
            retain: Retain::Tick,
            cadence_secs: None,
        }
    }

    fn obs() -> Observation {
        Observation {
            outcome: Outcome::Found {
                facts: Facts::new(json!({ "shape": "(a)->c" })),
            },
            fact_address: Some(FactAddress::new("b".repeat(64))),
            versions: versions(),
        }
    }

    fn at(n: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000 + n, 0).unwrap()
    }

    fn opened(terminal: &[&str], state: serde_json::Value) -> (Seq, Entry) {
        (
            1,
            Entry::Open {
                anchor: Box::new(anchor(terminal)),
                observation: obs(),
                state: State::new(state),
                at: at(0),
            },
        )
    }

    fn seal() -> ContentHash {
        ContentHash::new("e".repeat(64))
    }

    #[test]
    fn state_is_folded_not_stored() {
        let log = vec![
            opened(&[], json!({ STATUS: "ok" })),
            (
                2,
                Entry::Transition {
                    observation: obs(),
                    state: State::new(json!({ STATUS: "drifted" })),
                    at: at(10),
                },
            ),
        ];
        let s = fold(&log).unwrap();
        assert_eq!(s.state.status(), Some(StatusId::new("drifted")));
        assert_eq!(s.entered_at, Some(at(10)));
    }

    #[test]
    fn attempts_do_not_disturb_the_world() {
        let log = vec![
            opened(&[], json!({ STATUS: "ok" })),
            (
                2,
                Entry::Attempt {
                    reason: ReasonClass::Unreachable,
                    message: "boom".into(),
                    at: at(10),
                },
            ),
            (
                3,
                Entry::Attempt {
                    reason: ReasonClass::Unevaluable,
                    message: "no such field".into(),
                    at: at(20),
                },
            ),
        ];
        let s = fold(&log).unwrap();
        assert_eq!(s.attempts, 2);
        assert_eq!(s.state.status(), Some(StatusId::new("ok")), "状态没被动过");
        assert_eq!(s.entered_at, Some(at(0)), "我们的失败不重置世界的计时");
        assert_eq!(s.last_sighting, Some(at(0)));
    }

    #[test]
    fn a_sighting_clears_the_attempt_streak() {
        let log = vec![
            opened(&[], json!({ STATUS: "ok" })),
            (
                2,
                Entry::Attempt {
                    reason: ReasonClass::Unreachable,
                    message: "boom".into(),
                    at: at(10),
                },
            ),
            (
                3,
                Entry::Still {
                    ref_entry: 1,
                    at: at(20),
                    versions: versions(),
                },
            ),
        ];
        let s = fold(&log).unwrap();
        assert_eq!(s.attempts, 0);
        assert_eq!(s.last_sighting, Some(at(20)));
    }

    #[test]
    fn entering_a_terminal_state_closes_without_a_close_entry() {
        let log = vec![
            opened(&["settled"], json!({ STATUS: "pending" })),
            (
                2,
                Entry::Transition {
                    observation: obs(),
                    state: State::new(json!({ STATUS: "settled" })),
                    at: at(10),
                },
            ),
        ];
        assert!(fold(&log).unwrap().closed, "进了终结集合就是关了");
    }

    #[test]
    fn the_same_status_means_nothing_to_an_anchor_that_did_not_declare_it() {
        let log = vec![
            opened(&[], json!({ STATUS: "pending" })),
            (
                2,
                Entry::Transition {
                    observation: obs(),
                    state: State::new(json!({ STATUS: "settled" })),
                    at: at(10),
                },
            ),
        ];
        assert!(!fold(&log).unwrap().closed);
    }

    #[test]
    fn restate_is_how_an_author_accepts_a_change() {
        let log = vec![
            opened(&[], json!({ POSITION: "a.rs", STATUS: "ok" })),
            (
                2,
                Entry::Transition {
                    observation: obs(),
                    state: State::new(json!({ POSITION: "a.rs", STATUS: "drifted" })),
                    at: at(10),
                },
            ),
            (
                3,
                Entry::Revise {
                    change: Change::Restate {
                        state: State::new(json!({ POSITION: "b.rs", STATUS: "ok" })),
                    },
                    context: seal(),
                    rationale: seal(),
                    at: at(20),
                },
            ),
        ];
        let s = fold(&log).unwrap();
        assert_eq!(s.position(), &json!("b.rs"));
        assert_eq!(s.state.status(), Some(StatusId::new("ok")));
        assert_eq!(s.revisions.get("restate"), Some(&1));
        assert_eq!(s.entered_at, Some(at(20)));
    }

    #[test]
    fn reterminal_closes_an_anchor_already_sitting_in_that_state() {
        let log = vec![
            opened(&[], json!({ STATUS: "settled" })),
            (
                2,
                Entry::Revise {
                    change: Change::Reterminal {
                        terminal: BTreeSet::from([StatusId::new("settled")]),
                    },
                    context: seal(),
                    rationale: seal(),
                    at: at(10),
                },
            ),
        ];
        assert!(fold(&log).unwrap().closed);
    }

    #[test]
    fn a_still_moves_the_sighting_clock_but_not_the_observations_origin() {
        let log = vec![
            opened(&[], json!({ STATUS: "ok" })),
            (
                2,
                Entry::Still {
                    ref_entry: 1,
                    at: at(10),
                    versions: versions(),
                },
            ),
            (
                3,
                Entry::Still {
                    ref_entry: 1,
                    at: at(20),
                    versions: versions(),
                },
            ),
        ];
        let s = fold(&log).unwrap();
        assert_eq!(s.latest_seq, Some(1), "still 不带来新观测，出处还是 open");
        assert_eq!(s.last_sighting, Some(at(20)), "但我们确实又看了一次");
    }

    #[test]
    fn a_transition_moves_the_observations_origin() {
        let log = vec![
            opened(&[], json!({ STATUS: "ok" })),
            (
                7,
                Entry::Transition {
                    observation: obs(),
                    state: State::new(json!({ STATUS: "drifted" })),
                    at: at(10),
                },
            ),
        ];
        assert_eq!(fold(&log).unwrap().latest_seq, Some(7));
    }

    #[test]
    fn a_revise_moves_the_state_but_not_the_observations_origin() {
        let log = vec![
            opened(&[], json!({ POSITION: "a.rs", STATUS: "ok" })),
            (
                2,
                Entry::Revise {
                    change: Change::Restate {
                        state: State::new(json!({ POSITION: "b.rs", STATUS: "ok" })),
                    },
                    context: seal(),
                    rationale: seal(),
                    at: at(10),
                },
            ),
        ];
        let s = fold(&log).unwrap();
        assert_eq!(s.state.position(), &json!("b.rs"), "作者动了状态");
        assert_eq!(s.latest_seq, Some(1), "但没人重新观测过世界");
    }

    #[test]
    fn still_requires_both_the_state_and_the_facts_to_hold_put() {
        let s1 = State::new(json!({ STATUS: "ok" }));
        let s2 = State::new(json!({ STATUS: "drifted" }));
        let a = FactAddress::new("c".repeat(64));
        let b = FactAddress::new("d".repeat(64));

        assert!(should_still(&s1, Some(&a), &s1, Some(&a)));
        assert!(!should_still(&s1, Some(&a), &s2, Some(&a)), "状态动了");
        assert!(
            !should_still(&s1, Some(&a), &s1, Some(&b)),
            "世界沿没在看的方向动了 —— 留完整记录"
        );
    }

    #[test]
    fn entries_roundtrip_the_wire() {
        let e = Entry::Transition {
            observation: obs(),
            state: State::new(json!({ STATUS: "ok" })),
            at: at(0),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert_eq!(serde_json::from_str::<Entry>(&s).unwrap(), e);
    }
}

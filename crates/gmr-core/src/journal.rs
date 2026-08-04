use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::addr::ContentHash;
use crate::anchor::{Anchor, State, StatusId, Transitions};
use crate::probe::{Derivation, FactAddress, Facts, Outcome, ProbeRef};

pub type Seq = u64;

/// The three identities behind one observation. **Never merge them:** they
/// evolve independently and fail differently, so merging any two lies about one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versions {
    /// The sentence written on the anchor.
    pub declaration: ContentHash,
    /// What actually derived these facts, and whether that identity is provable.
    pub derivation: Derivation,
    /// The evaluator in force at the time.
    pub evaluator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub outcome: Outcome,
    pub fact_address: FactAddress,
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
    Reprobe { probe: ProbeRef },
    Retransition { transitions: Transitions },
    Reterminal { terminal: BTreeSet<StatusId> },
    Restate { state: State },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Reprobe,
    Retransition,
    Reterminal,
    Restate,
}

impl Change {
    pub fn kind(&self) -> ChangeKind {
        match self {
            Self::Reprobe { .. } => ChangeKind::Reprobe,
            Self::Retransition { .. } => ChangeKind::Retransition,
            Self::Reterminal { .. } => ChangeKind::Reterminal,
            Self::Restate { .. } => ChangeKind::Restate,
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

/// Why an observation did not land, at the granularity the failure was
/// actually known — [`ReasonClass`] is what the substrate acts on, this is
/// what a person needs to diagnose it.
///
/// Both halves of "our failure" are enumerated. A log that recorded seven
/// kinds of probe failure and one kind of rule failure would be describing
/// the tooling's history rather than the anchor's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    Unreachable,
    TimedOut,
    ProcessFailed,
    Unusable,
    ArtifactInvalid,
    OutputTooLarge,
    InvalidJson,
    Unparseable,
    GuardNotBoolean,
    NewStateNotAnObject,
    NewStateAbsent,
    NoSuchField,
    NotAnObject,
    NotAnArray,
    IndexOutOfRange,
    NotComparable,
    DividedByZero,
}

impl FailureCode {
    /// The class the substrate acts on. Derived rather than stored beside the
    /// code, so the two cannot come to disagree.
    pub fn reason(self) -> ReasonClass {
        match self {
            Self::Unreachable | Self::TimedOut | Self::ProcessFailed => ReasonClass::Unreachable,
            Self::Unusable | Self::ArtifactInvalid | Self::OutputTooLarge | Self::InvalidJson => {
                ReasonClass::Unusable
            }
            _ => ReasonClass::Unevaluable,
        }
    }
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
        /// Absent only in entries written before codes were recorded.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<FailureCode>,
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
    last_address: &FactAddress,
    now_state: &State,
    now_address: &FactAddress,
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
    pub revisions: BTreeMap<ChangeKind, u32>,
}

impl AnchorState {
    pub fn position(&self) -> &serde_json::Value {
        self.state.position()
    }
}

/// `closed` accumulates entry by entry and never clears: finishing is something
/// that happened in the log, not a re-reading of the final state.
pub fn fold(entries: &[(Seq, Entry)]) -> Option<AnchorState> {
    scan(entries, |_, _, _| {})
}

/// Walk the log once, handing over the fold as it stood after each entry.
///
/// `fold` is just its last cell. Consumers that need to know what happened along
/// the way come here and **do not write a second projection** — two projections
/// drift apart sooner or later, and nothing will notice.
pub fn scan(
    entries: &[(Seq, Entry)],
    mut each: impl FnMut(Seq, &Entry, &AnchorState),
) -> Option<AnchorState> {
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

        if let Some(s) = acc.as_mut() {
            s.closed = s.closed || s.anchor.is_terminal(&s.state);
            each(*seq, entry, s);
        }
    }

    acc
}

fn apply(s: &mut AnchorState, change: &Change, at: DateTime<Utc>) {
    *s.revisions.entry(change.kind()).or_insert(0) += 1;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::{AnchorKey, Expr, POSITION, Rule, STATUS};
    use crate::probe::{Kind, ProbeName, ProbeRef, ProbeVersion, Verifiability};
    use serde_json::json;

    fn versions() -> Versions {
        Versions {
            declaration: ContentHash::new("d".repeat(64)),
            derivation: Derivation {
                version: ProbeVersion::new("a".repeat(64)),
                verifiability: Verifiability::Closed,
            },
            evaluator: "eval-1".to_owned(),
        }
    }

    fn anchor(terminal: &[&str]) -> Anchor {
        Anchor {
            key: AnchorKey::new("a"),
            probe: ProbeRef::new(Kind::new("shell"), ProbeName::new("p"), json!({})),
            transitions: Transitions(vec![Rule {
                when: Expr::text("changed(\"shape\")"),
                to: Expr::text("{ status: \"drifted\" }"),
            }]),
            terminal: terminal.iter().map(|s| StatusId::new(*s)).collect(),
            supersedes: None,
        }
    }

    fn obs() -> Observation {
        Observation {
            outcome: Outcome::Found {
                facts: Facts::new(json!({ "shape": "(a)->c" })),
            },
            fact_address: FactAddress::new("b".repeat(64)),
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
                    code: None,
                    message: "boom".into(),
                    at: at(10),
                },
            ),
            (
                3,
                Entry::Attempt {
                    reason: ReasonClass::Unevaluable,
                    code: None,
                    message: "no such field".into(),
                    at: at(20),
                },
            ),
        ];
        let s = fold(&log).unwrap();
        assert_eq!(s.attempts, 2);
        assert_eq!(
            s.state.status(),
            Some(StatusId::new("ok")),
            "the state was never touched"
        );
        assert_eq!(
            s.entered_at,
            Some(at(0)),
            "our failures do not reset the world's clock"
        );
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
                    code: None,
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
        assert!(
            fold(&log).unwrap().closed,
            "landing in the terminal set is being closed"
        );
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
        assert_eq!(s.revisions.get(&ChangeKind::Restate), Some(&1));
        assert_eq!(s.entered_at, Some(at(20)));
    }

    #[test]
    fn counting_revisions_by_kind_stays_the_same_shape_on_the_wire() {
        let counted: BTreeMap<ChangeKind, u32> = [
            (ChangeKind::Reprobe, 1),
            (ChangeKind::Retransition, 2),
            (ChangeKind::Reterminal, 3),
            (ChangeKind::Restate, 4),
        ]
        .into();
        let wire = serde_json::to_value(&counted).unwrap();
        assert_eq!(
            wire,
            json!({ "reprobe": 1, "retransition": 2, "reterminal": 3, "restate": 4 })
        );
        assert_eq!(
            serde_json::from_value::<BTreeMap<ChangeKind, u32>>(wire).unwrap(),
            counted
        );
    }

    #[test]
    fn a_kind_is_the_variant_without_its_payload() {
        assert_eq!(
            Change::Restate {
                state: State::default()
            }
            .kind(),
            ChangeKind::Restate
        );
        assert_eq!(
            Change::Reterminal {
                terminal: BTreeSet::new()
            }
            .kind(),
            ChangeKind::Reterminal
        );
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
        assert_eq!(
            s.latest_seq,
            Some(1),
            "a still brings no new observation — the origin is still the open"
        );
        assert_eq!(s.last_sighting, Some(at(20)), "but we did look again");
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
        assert_eq!(
            s.state.position(),
            &json!("b.rs"),
            "the author moved the state"
        );
        assert_eq!(s.latest_seq, Some(1), "but nobody re-observed the world");
    }

    #[test]
    fn still_requires_both_the_state_and_the_facts_to_hold_put() {
        let s1 = State::new(json!({ STATUS: "ok" }));
        let s2 = State::new(json!({ STATUS: "drifted" }));
        let a = FactAddress::new("c".repeat(64));
        let b = FactAddress::new("d".repeat(64));

        assert!(should_still(&s1, &a, &s1, &a));
        assert!(!should_still(&s1, &a, &s2, &a), "the state moved");
        assert!(
            !should_still(&s1, &a, &s1, &b),
            "the world moved along a direction nobody watches — keep the full record"
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

    #[test]
    fn an_attempt_written_before_codes_existed_still_reads() {
        // The log is append-only, so entries already on disk have no `code`
        // and never will. They have to keep folding, not become unreadable.
        let old = json!({
            "entry": "attempt",
            "reason": "unreachable",
            "message": "boom",
            "at": at(10),
        });
        let e: Entry = serde_json::from_value(old).unwrap();
        let Entry::Attempt { reason, code, .. } = &e else {
            panic!("expected an attempt")
        };
        assert_eq!(*reason, ReasonClass::Unreachable);
        assert_eq!(*code, None, "absent, not guessed at");

        let s = fold(&[opened(&[], json!({ STATUS: "ok" })), (2, e)]).unwrap();
        assert_eq!(s.attempts, 1);
    }

    #[test]
    fn a_recorded_code_agrees_with_the_class_the_substrate_acts_on() {
        for code in [
            FailureCode::TimedOut,
            FailureCode::InvalidJson,
            FailureCode::NoSuchField,
            FailureCode::Unparseable,
        ] {
            let e = Entry::Attempt {
                reason: code.reason(),
                code: Some(code),
                message: "m".into(),
                at: at(0),
            };
            let back: Entry = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
            assert_eq!(back, e);
        }
        assert_eq!(FailureCode::TimedOut.reason(), ReasonClass::Unreachable);
        assert_eq!(FailureCode::InvalidJson.reason(), ReasonClass::Unusable);
        assert_eq!(FailureCode::NoSuchField.reason(), ReasonClass::Unevaluable);
        assert_eq!(FailureCode::Unparseable.reason(), ReasonClass::Unevaluable);
    }
}

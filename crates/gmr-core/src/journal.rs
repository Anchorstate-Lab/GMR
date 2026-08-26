use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::addr::ContentHash;
use crate::anchor::{Anchor, State, StatusId, Transitions};
use crate::probe::{Derivation, FactAddress, Facts, Outcome, ProbeRef};

pub type Seq = u64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versions {
    pub declaration: ContentHash,
    pub derivation: Derivation,
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
pub struct Faltering {
    pub attempts: u32,
    pub reason: ReasonClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<FailureCode>,
    pub message: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorState {
    pub anchor: Anchor,
    pub state: State,
    pub latest: Option<Observation>,
    pub latest_seq: Option<Seq>,
    pub closed: bool,
    pub faltering: Option<Faltering>,
    pub last_sighting: Option<DateTime<Utc>>,
    pub entered_at: Option<DateTime<Utc>>,
    pub moved_at: Option<Seq>,
    pub head: Seq,
    pub revisions: BTreeMap<ChangeKind, u32>,
}

impl AnchorState {
    pub fn position(&self) -> &serde_json::Value {
        self.state.position()
    }

    pub fn attempts(&self) -> u32 {
        self.faltering.as_ref().map_or(0, |f| f.attempts)
    }
}

pub fn fold(entries: &[(Seq, Entry)]) -> Option<AnchorState> {
    scan(entries, |_, _, _| {})
}

pub fn scan(
    entries: &[(Seq, Entry)],
    each: impl FnMut(Seq, &Entry, &AnchorState),
) -> Option<AnchorState> {
    resume(None, entries, each)
}

pub fn resume(
    from: Option<AnchorState>,
    entries: &[(Seq, Entry)],
    mut each: impl FnMut(Seq, &Entry, &AnchorState),
) -> Option<AnchorState> {
    let mut acc: Option<AnchorState> = from;

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
                    faltering: None,
                    last_sighting: Some(*at),
                    entered_at: Some(*at),
                    moved_at: Some(*seq),
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
                    s.moved_at = Some(*seq);
                }
                s.state = state.clone();
                s.latest = Some(observation.clone());
                s.latest_seq = Some(*seq);
                s.faltering = None;
                s.last_sighting = Some(*at);
                s.head = *seq;
            }
            Entry::Still { at, .. } => {
                let Some(s) = acc.as_mut() else { continue };
                s.faltering = None;
                s.last_sighting = Some(*at);
                s.head = *seq;
            }
            Entry::Attempt {
                reason,
                code,
                message,
                at,
            } => {
                let Some(s) = acc.as_mut() else { continue };
                s.faltering = Some(Faltering {
                    attempts: s.attempts() + 1,
                    reason: *reason,
                    code: *code,
                    message: message.clone(),
                    at: *at,
                });
                s.head = *seq;
            }
            Entry::Revise { change, at, .. } => {
                let Some(s) = acc.as_mut() else { continue };
                apply(s, change, *at, *seq);
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

fn apply(s: &mut AnchorState, change: &Change, at: DateTime<Utc>, seq: Seq) {
    *s.revisions.entry(change.kind()).or_insert(0) += 1;

    match change {
        Change::Reprobe { probe } => s.anchor.probe = probe.clone(),
        Change::Retransition { transitions } => s.anchor.transitions = transitions.clone(),
        Change::Reterminal { terminal } => s.anchor.terminal = terminal.clone(),
        Change::Restate { state } => {
            if s.state != *state {
                s.entered_at = Some(at);
                s.moved_at = Some(seq);
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
                version: ProbeVersion::try_new("a".repeat(64)).unwrap(),
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
            fact_address: FactAddress::try_new("b".repeat(64)).unwrap(),
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
        assert_eq!(s.attempts(), 2);
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
        assert_eq!(s.attempts(), 0);
        assert_eq!(
            s.faltering, None,
            "the count and the reason are one value, so a cleared streak cannot leave a \
             reason behind for the next reader to act on"
        );
        assert_eq!(s.last_sighting, Some(at(20)));
    }

    #[test]
    fn resuming_from_any_point_lands_where_folding_the_whole_log_lands() {
        let log = vec![
            opened(&["settled"], json!({ POSITION: "a.rs", STATUS: "open" })),
            (
                2,
                Entry::Attempt {
                    reason: ReasonClass::Unreachable,
                    code: Some(FailureCode::Unreachable),
                    message: "gone".to_owned(),
                    at: at(5),
                },
            ),
            (
                3,
                Entry::Transition {
                    observation: obs(),
                    state: State::new(json!({ POSITION: "b.rs", STATUS: "drifted" })),
                    at: at(10),
                },
            ),
            (
                4,
                Entry::Still {
                    ref_entry: 3,
                    at: at(15),
                    versions: versions(),
                },
            ),
            (
                5,
                Entry::Revise {
                    change: Change::Restate {
                        state: State::new(json!({ POSITION: "c.rs", STATUS: "ok" })),
                    },
                    context: seal(),
                    rationale: seal(),
                    at: at(20),
                },
            ),
            (
                6,
                Entry::Close {
                    at: at(25),
                    context: seal(),
                    rationale: seal(),
                },
            ),
        ];

        let whole = fold(&log).expect("a log that opens folds");

        for cut in 1..=log.len() {
            let checkpoint = fold(&log[..cut]).expect("every prefix opens too");
            assert_eq!(
                resume(Some(checkpoint), &log[cut..], |_, _, _| {}),
                Some(whole.clone()),
                "a checkpoint taken after {cut} entries and carried forward over the rest \
                 must land exactly where folding all {} lands. This is the whole licence \
                 for caching a fold: on an append-only log a state at a seq only ever goes \
                 stale, never wrong, so catching it up needs no invalidation. The property \
                 holds because every arm of this fold reads only the accumulator and the \
                 entry -- `Still` carries `ref_entry` and does not follow it. An arm that \
                 reached back for an earlier entry would break this without breaking any \
                 test that folds from zero",
                log.len()
            );
        }
    }

    #[test]
    fn the_clock_and_the_cursor_mark_the_same_move() {
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
                Entry::Transition {
                    observation: obs(),
                    state: State::new(json!({ STATUS: "ok" })),
                    at: at(20),
                },
            ),
            (
                4,
                Entry::Transition {
                    observation: obs(),
                    state: State::new(json!({ STATUS: "drifted" })),
                    at: at(30),
                },
            ),
        ];

        let s = fold(&log).unwrap();

        assert_eq!(
            (s.entered_at, s.moved_at),
            (Some(at(30)), Some(4)),
            "`entered_at` and `moved_at` are one event in two units -- when the state \
             changed, and where in the log it changed. Set one without the other and a \
             reader comparing seqs disagrees with a reader comparing clocks about whether \
             the ground moved"
        );
        assert_eq!(
            s.head, 4,
            "the head is every entry; the move is only the entries that changed the state. \
             A failed attempt and a transition that restated the same value both advance \
             the head and neither is the ground moving"
        );
    }

    #[test]
    fn a_streak_carries_the_reason_of_its_latest_failure() {
        let log = vec![
            opened(&[], json!({ STATUS: "ok" })),
            (
                2,
                Entry::Attempt {
                    reason: ReasonClass::Unreachable,
                    code: Some(FailureCode::TimedOut),
                    message: "boom".into(),
                    at: at(10),
                },
            ),
            (
                3,
                Entry::Attempt {
                    reason: ReasonClass::Unevaluable,
                    code: Some(FailureCode::NoSuchField),
                    message: "no such field".into(),
                    at: at(20),
                },
            ),
        ];

        let f = fold(&log)
            .unwrap()
            .faltering
            .expect("two failures and no sighting since is a streak");

        assert_eq!(
            (f.reason, f.code),
            (ReasonClass::Unevaluable, Some(FailureCode::NoSuchField)),
            "a fold that keeps only a count tells a reader that something failed twice and \
             not whether the source could not be reached or the rules could not be \
             evaluated -- which are different people's problem. `check` escapes this only \
             because it observes afresh and reads the code off the live outcome; anything \
             answering from folded state has no second chance to ask"
        );
        assert_eq!(f.at, at(20), "the streak is dated by its latest failure");
        assert_eq!(f.attempts, 2);
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
        let a = FactAddress::try_new("c".repeat(64)).unwrap();
        let b = FactAddress::try_new("d".repeat(64)).unwrap();

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
        assert_eq!(s.attempts(), 1);
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

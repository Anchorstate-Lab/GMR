use std::sync::Arc;

use gmr_core::{
    AnchorKey, Change, Expr, Kind, ProbeRef, ReasonClass, Retain, Rule, RunSettings, State,
    StatusId, Transitions, fold,
};
use gmr_runtime::{Observed, OpenRequest, Runtime, Sighting};
use gmr_store::testkit::{MemoryBindings, MemoryJournal, MemoryQueue};
use gmr_transport::shell::Shell;

fn script_probe(root: &std::path::Path, name: &str, body: &str) -> gmr_core::ProbeRef {
    gmr_transport::shell::testkit::install_script(root.join(".probes"), name, body)
}

fn cat_probe(root: &std::path::Path) -> gmr_core::ProbeRef {
    script_probe(root, "cat", "cat world.json")
}

struct World {
    dir: tempfile::TempDir,
    rt: Runtime,
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let bindings = Arc::new(MemoryBindings::default());
        let rt = Runtime::builder()
            .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(bindings.clone())
            .sealer(bindings.clone())
            .links(bindings)
            .settings(Arc::new(MemoryQueue::default()))
            .build();
        Self { dir, rt }
    }

    fn write(&self, contents: &str) {
        std::fs::write(self.dir.path().join("world.json"), contents).unwrap();
    }

    fn remove(&self) {
        let _ = std::fs::remove_file(self.dir.path().join("world.json"));
    }

    async fn open(&self, rules: &[(&str, &str)], terminal: &[&str]) -> State {
        self.rt
            .open(OpenRequest {
                key: key(),
                probe: cat_probe(self.dir.path()),
                transitions: transitions(rules),
                terminal: terminal.iter().map(|s| StatusId::new(*s)).collect(),
                initial: None,
                settings: RunSettings {
                    budget_ms: None,
                    retain: Retain::Tick,
                    cadence_secs: None,
                },
                supersedes: None,
            })
            .await
            .unwrap()
            .state
    }

    async fn observe(&self) -> Observed {
        self.rt.observe(&key()).await.unwrap()
    }

    async fn state(&self) -> State {
        self.rt.read(&key()).await.unwrap().state
    }

    async fn status(&self) -> Option<String> {
        self.rt
            .read(&key())
            .await
            .unwrap()
            .status
            .map(|s| s.to_string())
    }
}

fn key() -> AnchorKey {
    AnchorKey::new("a")
}

fn transitions(pairs: &[(&str, &str)]) -> Transitions {
    Transitions(
        pairs
            .iter()
            .map(|(w, t)| Rule {
                when: Expr::text(*w),
                to: Expr::text(*t),
            })
            .collect(),
    )
}

fn moved(o: &Observed) -> bool {
    matches!(o, Observed::Transitioned { .. })
}

#[tokio::test]
async fn an_anchor_that_declares_no_rule_still_records_that_the_world_moved() {
    let w = World::new();
    w.write(r#"{"shape":"(a)->c"}"#);
    w.open(&[], &[]).await;
    assert_eq!(w.status().await, None, "nobody declared a status");

    assert_eq!(w.observe().await, Observed::Still, "the world held still");

    w.write(r#"{"shape":"(a,b)->c"}"#);
    assert!(
        matches!(w.observe().await, Observed::Unchanged { .. }),
        "the facts moved, so a full entry is kept — but no rule matched, so the state did not"
    );
    assert_eq!(w.state().await, State::default());
}

#[tokio::test]
async fn a_declared_direction_moves_the_machine() {
    let w = World::new();
    w.write(r#"{"shape":"(a)->c"}"#);
    w.open(
        &[(
            r#"changed("shape")"#,
            r#"{ shape: obs.shape, status: "drifted" }"#,
        )],
        &[],
    )
    .await;

    w.write(r#"{"shape":"(a,b)->c"}"#);
    assert!(moved(&w.observe().await));
    assert_eq!(w.status().await.as_deref(), Some("drifted"));
    assert_eq!(w.state().await.as_value()["shape"], "(a,b)->c");
}

#[tokio::test]
async fn an_undeclared_direction_moves_nothing() {
    let w = World::new();
    w.write(r#"{"shape":"(a)->c","body":"one"}"#);
    w.open(&[(r#"changed("shape")"#, r#"{ shape: obs.shape }"#)], &[])
        .await;

    w.write(r#"{"shape":"(a)->c","body":"two"}"#);
    assert!(
        !moved(&w.observe().await),
        "implementation changed, but nobody declared that direction"
    );
}

#[tokio::test]
async fn the_first_matching_rule_wins() {
    let w = World::new();
    w.write(r#"{"n":1}"#);
    w.open(
        &[
            ("obs.n > 10", r#"{ n: obs.n, status: "big" }"#),
            ("obs.n > 1", r#"{ n: obs.n, status: "small" }"#),
        ],
        &[],
    )
    .await;

    w.write(r#"{"n":50}"#);
    w.observe().await;
    assert_eq!(w.status().await.as_deref(), Some("big"));
}

#[tokio::test]
async fn changing_back_can_be_expressed_as_going_back() {
    let w = World::new();
    w.write(r#"{"shape":"(a)->c"}"#);
    w.open(
        &[
            (r#"obs.shape == "(a)->c""#, r#"{ status: "ok" }"#),
            ("true", r#"{ status: "drifted" }"#),
        ],
        &[],
    )
    .await;

    w.write(r#"{"shape":"(a,b)->c"}"#);
    w.observe().await;
    assert_eq!(w.status().await.as_deref(), Some("drifted"));

    w.write(r#"{"shape":"(a)->c"}"#);
    w.observe().await;
    assert_eq!(
        w.status().await.as_deref(),
        Some("ok"),
        "changing back self-heals"
    );
}

#[tokio::test]
async fn a_terminal_state_stops_the_machine_for_good() {
    let w = World::new();
    w.write(r#"{"n":1}"#);
    w.open(&[("obs.n > 10", r#"{ status: "settled" }"#)], &["settled"])
        .await;

    w.write(r#"{"n":50}"#);
    w.observe().await;
    assert_eq!(w.status().await.as_deref(), Some("settled"));

    w.write(r#"{"n":1}"#);
    assert_eq!(w.observe().await, Observed::Closed);
    assert_eq!(
        w.status().await.as_deref(),
        Some("settled"),
        "it cannot go back"
    );
}

#[tokio::test]
async fn the_terminal_set_belongs_to_the_anchor_not_to_the_word() {
    let w = World::new();
    w.write(r#"{"n":50}"#);
    w.open(&[("obs.n > 10", r#"{ status: "settled" }"#)], &[])
        .await;

    w.write(r#"{"n":1}"#);
    assert_ne!(w.observe().await, Observed::Closed);
}

#[tokio::test]
async fn the_substrate_never_reads_into_a_status() {
    let w = World::new();
    w.write(r#"{"n":50}"#);
    w.open(
        &[("obs.n > 10", r#"{ status: "settled-local" }"#)],
        &["settled-local"],
    )
    .await;
    assert_eq!(w.observe().await, Observed::Closed);
}

#[tokio::test]
async fn the_position_reaches_the_probe_and_the_domain_can_move_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.json"), r#"{"v":1}"#).unwrap();
    std::fs::write(dir.path().join("b.json"), r#"{"v":2}"#).unwrap();

    let bindings = Arc::new(MemoryBindings::default());
    let rt = Runtime::builder()
        .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
        .journal(Arc::new(MemoryJournal::default()))
        .bindings(bindings.clone())
        .sealer(bindings.clone())
        .links(bindings)
        .settings(Arc::new(MemoryQueue::default()))
        .build();

    let opened = rt
        .open(OpenRequest {
            key: key(),
            probe: script_probe(
                dir.path(),
                "at-position",
                r#"cat "$(echo "$GMR_POSITION" | tr -d '"')""#,
            ),
            transitions: transitions(&[("true", "{ position: state.position, v: obs.v }")]),
            terminal: Default::default(),
            initial: Some(State::new(serde_json::json!({ "position": "a.json" }))),
            settings: RunSettings {
                budget_ms: None,
                retain: Retain::Tick,
                cadence_secs: None,
            },
            supersedes: None,
        })
        .await
        .unwrap();
    assert_eq!(opened.state.as_value()["v"], 1);

    rt.revise(
        &key(),
        Change::Restate {
            state: State::new(serde_json::json!({ "position": "b.json" })),
        },
        "the watched object moved".as_bytes(),
    )
    .await
    .unwrap();

    rt.observe(&key()).await.unwrap();
    assert_eq!(rt.read(&key()).await.unwrap().state.as_value()["v"], 2);
}

#[tokio::test]
async fn a_probe_that_cannot_run_is_our_failure_not_the_worlds_answer() {
    let w = World::new();
    w.write(r#"{"n":1}"#);
    w.open(&[("true", "{ n: obs.n }")], &[]).await;

    w.remove();
    let o = w.observe().await;
    assert!(
        matches!(&o, Observed::Attempt { reason, .. } if *reason == ReasonClass::Unreachable),
        "{o:?}"
    );
    assert_eq!(w.state().await.as_value()["n"], 1);
}

#[tokio::test]
async fn a_transition_that_cannot_be_evaluated_must_be_loud() {
    let w = World::new();
    w.write(r#"{"shape":"(a)->c"}"#);
    w.open(&[(r#"obs.shape != "x""#, "{ shape: obs.shape }")], &[])
        .await;

    w.write(r#"{"renamed":"(a)->c"}"#);
    let o = w.observe().await;
    assert!(
        matches!(&o, Observed::Attempt { reason, .. } if *reason == ReasonClass::Unevaluable),
        "{o:?}"
    );
}

#[tokio::test]
async fn the_world_being_empty_is_a_real_answer_and_it_lands_as_an_entry() {
    let dir = tempfile::tempdir().unwrap();
    let bindings = Arc::new(MemoryBindings::default());
    let rt = Runtime::builder()
        .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
        .journal(Arc::new(MemoryJournal::default()))
        .bindings(bindings.clone())
        .sealer(bindings.clone())
        .links(bindings)
        .settings(Arc::new(MemoryQueue::default()))
        .build();

    rt.open(OpenRequest {
        key: key(),
        probe: script_probe(dir.path(), "absent", "echo null"),
        transitions: transitions(&[("true", r#"{ status: "empty" }"#)]),
        terminal: Default::default(),
        initial: None,
        settings: RunSettings {
            budget_ms: None,
            retain: Retain::Tick,
            cadence_secs: None,
        },
        supersedes: None,
    })
    .await
    .unwrap();

    let view = rt.read(&key()).await.unwrap();
    assert_eq!(view.sighting, Sighting::Absent);
    assert_eq!(view.status.map(|s| s.to_string()).as_deref(), Some("empty"));
    assert_eq!(
        view.attempts, 0,
        "this is a fact about the world, not our failure"
    );
}

#[tokio::test]
async fn nothing_moving_writes_a_still_not_a_transition() {
    let w = World::new();
    w.write(r#"{"n":1}"#);
    w.open(&[("changed(\"n\")", "{ n: obs.n }")], &[]).await;

    assert_eq!(w.observe().await, Observed::Still);
    assert_eq!(w.observe().await, Observed::Still);

    let entries = w.rt.log().entries(&key(), 0).await.unwrap();
    let stills = entries.iter().filter(|(_, e)| e.name() == "still").count();
    assert_eq!(stills, 2);
    assert_eq!(fold(&entries).unwrap().attempts, 0);
}

#[tokio::test]
async fn the_domain_counts_for_itself_and_picks_its_own_yardstick() {
    let w = World::new();
    w.write(r#"{"v":1}"#);
    w.open(
        &[
            (
                "state.n >= 2",
                r#"{ v: obs.v, n: state.n + 1, status: "confirmed" }"#,
            ),
            ("changed(\"v\")", "{ v: obs.v, n: 0 }"),
            ("true", "{ v: obs.v, n: state.n + 1 }"),
        ],
        &[],
    )
    .await;

    w.write(r#"{"v":2}"#);
    w.observe().await;
    w.observe().await;
    assert_eq!(w.status().await, None);
    w.observe().await;
    w.observe().await;
    assert_eq!(w.status().await.as_deref(), Some("confirmed"));
}

#[tokio::test]
async fn the_domain_must_capture_its_own_starting_point() {
    let w = World::new();
    w.write(r#"{"shape":"(a)->c"}"#);
    w.open(
        &[(
            "not exists(state.shape)",
            r#"{ shape: obs.shape, status: "captured" }"#,
        )],
        &[],
    )
    .await;
    assert_eq!(w.state().await.as_value()["shape"], "(a)->c");
    assert_eq!(w.status().await.as_deref(), Some("captured"));
}

#[tokio::test]
async fn partial_acceptance_is_written_out_in_the_open() {
    let w = World::new();
    w.write(r#"{"at":"a.rs","shape":"(a)->c"}"#);
    w.open(
        &[
            (
                "not exists(state.shape)",
                r#"{ at: obs.at, shape: obs.shape, status: "ok" }"#,
            ),
            (
                "changed(\"shape\") or changed(\"at\")",
                r#"{ at: obs.at, shape: state.shape, status: "drifted" }"#,
            ),
        ],
        &[],
    )
    .await;

    w.write(r#"{"at":"b.rs","shape":"(a,b)->c"}"#);
    w.observe().await;

    let s = w.state().await;
    assert_eq!(s.as_value()["at"], "b.rs", "accepted the move");
    assert_eq!(
        s.as_value()["shape"],
        "(a)->c",
        "did not accept the signature change"
    );
    assert_eq!(
        w.status().await.as_deref(),
        Some("drifted"),
        "and it knows the current state is assembled"
    );
}

#[tokio::test]
async fn accepting_a_change_by_hand_is_sealed() {
    let w = World::new();
    w.write(r#"{"shape":"(a)->c"}"#);
    w.open(
        &[(
            "changed(\"shape\")",
            r#"{ shape: obs.shape, status: "drifted" }"#,
        )],
        &[],
    )
    .await;

    w.write(r#"{"shape":"(a,b)->c"}"#);
    w.observe().await;

    let revised =
        w.rt.revise(
            &key(),
            Change::Restate {
                state: State::new(serde_json::json!({ "shape": "(a,b)->c", "status": "ok" })),
            },
            "reasonable evolution, signature should change".as_bytes(),
        )
        .await
        .unwrap();

    assert_eq!(w.status().await.as_deref(), Some("ok"));
    assert_ne!(revised.context, revised.rationale);
    assert!(
        w.rt.memory()
            .sealed(&revised.rationale)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn a_world_that_did_not_move_stays_still_even_right_after_a_restate() {
    let w = World::new();
    w.write(r#"{"shape":"(a)->c"}"#);
    w.open(
        &[(
            "changed(\"shape\")",
            r#"{ shape: obs.shape, status: "drifted" }"#,
        )],
        &[],
    )
    .await;

    w.rt.revise(
        &key(),
        Change::Restate {
            state: State::new(serde_json::json!({ "shape": "(a)->c", "status": "ok" })),
        },
        "accept current state".as_bytes(),
    )
    .await
    .unwrap();

    assert_eq!(
        w.observe().await,
        Observed::Still,
        "the world did not move; an author restate is not a world transition"
    );
    assert_eq!(w.status().await.as_deref(), Some("ok"));
}

#[tokio::test]
async fn every_still_in_a_run_points_at_the_record_it_was_compared_against() {
    let w = World::new();
    w.write(r#"{"shape":"(a)->c"}"#);
    w.open(&[("changed(\"shape\")", r#"{ shape: obs.shape }"#)], &[])
        .await;

    for _ in 0..3 {
        assert_eq!(w.observe().await, Observed::Still);
    }

    let refs: Vec<u64> =
        w.rt.log()
            .entries(&key(), 0)
            .await
            .unwrap()
            .into_iter()
            .filter_map(|(_, e)| match e {
                gmr_core::Entry::Still { ref_entry, .. } => Some(ref_entry),
                _ => None,
            })
            .collect();

    assert_eq!(
        refs,
        vec![1, 1, 1],
        "each still points back to the full record, not to a chain of still entries"
    );
}

#[tokio::test]
async fn restate_cannot_resurrect_a_finished_anchor() {
    let w = World::new();
    w.write(r#"{"n":50}"#);
    w.open(&[("obs.n > 10", r#"{ status: "settled" }"#)], &["settled"])
        .await;
    assert_eq!(w.observe().await, Observed::Closed);

    let e =
        w.rt.revise(
            &key(),
            Change::Restate {
                state: State::new(serde_json::json!({ "status": "pending" })),
            },
            "want to take it back".as_bytes(),
        )
        .await
        .expect_err("state cannot be changed after closure");
    assert_eq!(e.code(), "anchor_closed");

    assert_eq!(w.status().await.as_deref(), Some("settled"));
    assert_eq!(w.observe().await, Observed::Closed);
}

#[tokio::test]
async fn emptying_the_terminal_set_cannot_resurrect_a_finished_anchor() {
    let w = World::new();
    w.write(r#"{"n":50}"#);
    w.open(&[("obs.n > 10", r#"{ status: "settled" }"#)], &["settled"])
        .await;
    assert_eq!(w.observe().await, Observed::Closed);

    w.rt.revise(
        &key(),
        Change::Reterminal {
            terminal: Default::default(),
        },
        "never mind".as_bytes(),
    )
    .await
    .expect_err("a wrong criterion needs a new generation, not resurrection");

    assert!(w.rt.read(&key()).await.unwrap().closed);
}

#[tokio::test]
async fn an_author_sealed_close_is_equally_final() {
    let w = World::new();
    w.write(r#"{"n":1}"#);
    w.open(&[], &[]).await;
    w.rt.close(&key(), "done".as_bytes()).await.unwrap();

    w.rt.close(&key(), "again".as_bytes())
        .await
        .expect_err("a closed anchor cannot be closed again");
    w.rt.revise(
        &key(),
        Change::Restate {
            state: State::new(serde_json::json!({ "status": "back" })),
        },
        "come back".as_bytes(),
    )
    .await
    .expect_err("a closed anchor cannot be changed");
}

#[tokio::test]
async fn a_terminal_transition_is_remembered_even_after_the_state_moves_on() {
    use gmr_core::{Entry, Observation, Outcome, Versions, fold};

    let anchor = gmr_core::Anchor {
        key: key(),
        probe: ProbeRef::new(
            Kind::new("shell"),
            gmr_core::ProbeName::new("p"),
            serde_json::json!({}),
        ),
        transitions: Transitions::default(),
        terminal: [StatusId::new("settled")].into_iter().collect(),
        supersedes: None,
    };
    let observation = Observation {
        outcome: Outcome::NotFound,
        fact_address: gmr_core::FactAddress::new("b".repeat(64)),
        versions: Versions {
            declaration: gmr_core::ContentHash::new("d".repeat(64)),
            derivation: gmr_core::Derivation {
                version: gmr_core::ProbeVersion::new("a".repeat(64)),
                verifiability: gmr_core::Verifiability::Closed,
            },
            evaluator: "e".to_owned(),
        },
    };
    let at = |n: i64| chrono::DateTime::from_timestamp(1_700_000_000 + n, 0).unwrap();

    let log = vec![
        (
            1,
            Entry::Open {
                anchor: Box::new(anchor),
                observation: observation.clone(),
                state: State::new(serde_json::json!({ "status": "pending" })),
                at: at(0),
            },
        ),
        (
            2,
            Entry::Transition {
                observation,
                state: State::new(serde_json::json!({ "status": "settled" })),
                at: at(10),
            },
        ),
        (
            3,
            Entry::Revise {
                change: Change::Restate {
                    state: State::new(serde_json::json!({ "status": "pending" })),
                },
                context: gmr_core::ContentHash::new("e".repeat(64)),
                rationale: gmr_core::ContentHash::new("f".repeat(64)),
                at: at(20),
            },
        ),
    ];

    assert!(
        fold(&log).unwrap().closed,
        "the log once entered the terminal set; later entries cannot change that fact"
    );
}

#[tokio::test]
async fn a_new_generation_supersedes_the_finished_one_with_a_sealed_reason() {
    use gmr_runtime::Supersede;

    let w = World::new();
    w.write(r#"{"n":50}"#);
    w.open(&[("obs.n > 10", r#"{ status: "settled" }"#)], &["settled"])
        .await;
    assert!(w.rt.read(&key()).await.unwrap().closed);

    let heir = AnchorKey::new("a@2");
    let opened =
        w.rt.open(OpenRequest {
            key: heir.clone(),
            probe: cat_probe(w.dir.path()),
            transitions: transitions(&[("obs.n > 100", r#"{ status: "settled" }"#)]),
            terminal: [StatusId::new("settled")].into_iter().collect(),
            initial: None,
            settings: RunSettings {
                budget_ms: None,
                retain: Retain::Tick,
                cadence_secs: None,
            },
            supersedes: Some(Supersede {
                key: key(),
                rationale: "threshold was wrong; 10 was too low".as_bytes().to_vec(),
            }),
        })
        .await
        .unwrap();

    assert_eq!(opened.supersedes.as_ref(), Some(&key()));

    let cited = w.rt.read(&heir).await.unwrap().anchor.supersedes.unwrap();
    assert_eq!(cited.key, key());
    assert_eq!(
        w.rt.memory().sealed(&cited.rationale).await.unwrap(),
        Some("threshold was wrong; 10 was too low".as_bytes().to_vec()),
    );

    assert!(w.rt.read(&key()).await.unwrap().closed);
}

#[tokio::test]
async fn an_anchor_still_running_cannot_be_superseded() {
    use gmr_runtime::Supersede;

    let w = World::new();
    w.write(r#"{"n":1}"#);
    w.open(&[], &[]).await;

    let e =
        w.rt.open(OpenRequest {
            key: AnchorKey::new("a@2"),
            probe: cat_probe(w.dir.path()),
            transitions: Transitions::default(),
            terminal: Default::default(),
            initial: None,
            settings: RunSettings {
                budget_ms: None,
                retain: Retain::Tick,
                cadence_secs: None,
            },
            supersedes: Some(Supersede {
                key: key(),
                rationale: b"why".to_vec(),
            }),
        })
        .await
        .expect_err("running anchor cannot be superseded");
    assert_eq!(e.code(), "not_closed_yet");
}

#[tokio::test]
async fn a_direction_that_has_not_grown_yet_warns_instead_of_refusing() {
    let w = World::new();
    w.write("{}");
    let opened =
        w.rt.open(OpenRequest {
            key: key(),
            probe: cat_probe(w.dir.path()),
            transitions: transitions(&[(r#"changed("shape")"#, r#"{ shape: obs.shape }"#)]),
            terminal: Default::default(),
            initial: None,
            settings: RunSettings {
                budget_ms: None, retain: Retain::Tick, cadence_secs: None },
            supersedes: None,
        })
        .await
        .expect("a misspelling and a not-yet-grown target look the same at opening time, so neither should block opening");

    assert!(
        opened.warnings.iter().any(|w| w.contains("no_such_field")),
        "but it must be reported: {:?}",
        opened.warnings
    );

    w.write(r#"{"shape":"(a)->c"}"#);
    assert!(moved(&w.observe().await));
    assert_eq!(w.state().await.as_value()["shape"], "(a)->c");
}

#[tokio::test]
async fn a_misspelt_direction_is_loud_at_the_first_real_observation() {
    let w = World::new();
    w.write(r#"{"signature":"(a)->c"}"#);
    w.open(&[(r#"changed("signatur")"#, r#"{ x: 1 }"#)], &[])
        .await;

    let seen = w.observe().await;
    let Observed::Attempt {
        reason, message, ..
    } = &seen
    else {
        panic!("a misspelled direction must be loud, not silently false forever: {seen:?}")
    };
    assert_eq!(*reason, gmr_core::ReasonClass::Unevaluable);
    assert!(message.contains("no_such_field"), "{message}");
}

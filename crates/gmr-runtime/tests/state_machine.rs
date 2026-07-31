use std::sync::Arc;

use gmr_core::{
    AnchorKey, Change, Expr, Kind, Probe, ReasonClass, Retain, Rule, State, StatusId, Transitions,
    fold,
};
use gmr_runtime::{Observed, OpenRequest, Runtime, Sighting};
use gmr_store::testkit::{MemoryBindings, MemoryJournal};
use gmr_transport_shell::Shell;

struct World {
    dir: tempfile::TempDir,
    rt: Runtime,
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let rt = Runtime::builder()
            .transport(Arc::new(Shell::new(dir.path())))
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(Arc::new(MemoryBindings::default()))
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
                probe: Probe::new(
                    Kind::new("shell"),
                    serde_json::json!({ "run": "cat world.json" }),
                ),
                transitions: transitions(rules),
                terminal: terminal.iter().map(|s| StatusId::new(*s)).collect(),
                initial: None,
                retain: Retain::Tick,
                cadence_secs: None,
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
    matches!(o, Observed::Transitioned { from, to } if from != to)
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
    assert!(!moved(&w.observe().await), "实现变了，但没人声明在看它");
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
    assert_eq!(w.status().await.as_deref(), Some("ok"), "改回去就自愈");
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
    assert_eq!(w.status().await.as_deref(), Some("settled"), "回不去了");
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
    w.open(&[("obs.n > 10", r#"{ status: "已结算" }"#)], &["已结算"])
        .await;
    assert_eq!(w.observe().await, Observed::Closed);
}

#[tokio::test]
async fn the_position_reaches_the_probe_and_the_domain_can_move_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.json"), r#"{"v":1}"#).unwrap();
    std::fs::write(dir.path().join("b.json"), r#"{"v":2}"#).unwrap();

    let rt = Runtime::builder()
        .transport(Arc::new(Shell::new(dir.path())))
        .journal(Arc::new(MemoryJournal::default()))
        .bindings(Arc::new(MemoryBindings::default()))
        .build();

    let opened = rt
        .open(OpenRequest {
            key: key(),
            probe: Probe::new(
                Kind::new("shell"),
                serde_json::json!({ "run": r#"cat "$(echo $GMR_POSITION | tr -d '"')""# }),
            ),
            transitions: transitions(&[("true", "{ position: state.position, v: obs.v }")]),
            terminal: Default::default(),
            initial: Some(State::new(serde_json::json!({ "position": "a.json" }))),
            retain: Retain::Tick,
            cadence_secs: None,
        })
        .await
        .unwrap();
    assert_eq!(opened.state.as_value()["v"], 1);

    rt.revise(
        &key(),
        Change::Restate {
            state: State::new(serde_json::json!({ "position": "b.json" })),
        },
        "盯的东西搬家了".as_bytes(),
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
    let rt = Runtime::builder()
        .transport(Arc::new(Shell::new(dir.path())))
        .journal(Arc::new(MemoryJournal::default()))
        .bindings(Arc::new(MemoryBindings::default()))
        .build();

    rt.open(OpenRequest {
        key: key(),
        probe: Probe::new(
            Kind::new("shell"),
            serde_json::json!({ "run": "echo null" }),
        ),
        transitions: transitions(&[("true", r#"{ status: "empty" }"#)]),
        terminal: Default::default(),
        initial: None,
        retain: Retain::Tick,
        cadence_secs: None,
    })
    .await
    .unwrap();

    let view = rt.read(&key()).await.unwrap();
    assert_eq!(view.sighting, Sighting::Absent);
    assert_eq!(view.status.map(|s| s.to_string()).as_deref(), Some("empty"));
    assert_eq!(view.attempts, 0, "这是关于世界的事实，不是我们的失败");
}

#[tokio::test]
async fn nothing_moving_writes_a_still_not_a_transition() {
    let w = World::new();
    w.write(r#"{"n":1}"#);
    w.open(&[("changed(\"n\")", "{ n: obs.n }")], &[]).await;

    assert_eq!(w.observe().await, Observed::Still);
    assert_eq!(w.observe().await, Observed::Still);

    let entries = w.rt.journal().entries(&key(), 0).await.unwrap();
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
    assert_eq!(s.as_value()["at"], "b.rs", "接受了搬家");
    assert_eq!(s.as_value()["shape"], "(a)->c", "没接受签名变化");
    assert_eq!(
        w.status().await.as_deref(),
        Some("drifted"),
        "而且它知道自己现在是拼接出来的"
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
            "合理演进，签名该变".as_bytes(),
        )
        .await
        .unwrap();

    assert_eq!(w.status().await.as_deref(), Some("ok"));
    assert_ne!(revised.context, revised.rationale);
    assert!(
        w.rt.bindings()
            .sealed(&revised.rationale)
            .await
            .unwrap()
            .is_some()
    );
}

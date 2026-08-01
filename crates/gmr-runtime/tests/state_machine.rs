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
        supersedes: None,
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
        "接受当下".as_bytes(),
    )
    .await
    .unwrap();

    assert_eq!(
        w.observe().await,
        Observed::Still,
        "世界一动没动 —— 作者改了状态不是世界的一次转换"
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
        w.rt.journal()
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
        "每条 still 都指回那条完整记录，而不是串成一条链"
    );
}

// ── 终结是结构，不是当下这一次解释 ──────────────────────────────
//
// 「不可逆」如果只在 fold 的最后一行按最后一个状态算一次，那它就是
// 一个视图，不是事实：任何把状态挪出终结集合的动作都能静默复活它。

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
            "想反悔".as_bytes(),
        )
        .await
        .expect_err("终结之后不许再改状态");
    assert!(e.to_string().contains("终结"), "{e}");

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
        "算了".as_bytes(),
    )
    .await
    .expect_err("判据写错了要开新一代，不是把这个锚拽回来");

    assert!(w.rt.read(&key()).await.unwrap().closed);
}

#[tokio::test]
async fn an_author_sealed_close_is_equally_final() {
    let w = World::new();
    w.write(r#"{"n":1}"#);
    w.open(&[], &[]).await;
    w.rt.close(&key(), "收工".as_bytes()).await.unwrap();

    w.rt.close(&key(), "再来一次".as_bytes())
        .await
        .expect_err("关过的锚不能再关");
    w.rt.revise(
        &key(),
        Change::Restate {
            state: State::new(serde_json::json!({ "status": "back" })),
        },
        "回来".as_bytes(),
    )
    .await
    .expect_err("关过的锚不能再改");
}

#[tokio::test]
async fn a_terminal_transition_is_remembered_even_after_the_state_moves_on() {
    // 直接喂日志：确认粘性住在 fold 里，不是住在 revise 的守卫里。
    use gmr_core::{Entry, Observation, Outcome, ProbeVersion, Versions, fold};

    let anchor = gmr_core::Anchor {
        key: key(),
        probe: Probe::new(Kind::new("shell"), serde_json::json!({ "run": "x" })),
        transitions: Transitions::default(),
        terminal: [StatusId::new("settled")].into_iter().collect(),
        retain: Retain::Tick,
        cadence_secs: None,
        supersedes: None,
    };
    let observation = Observation {
        outcome: Outcome::NotFound,
        fact_address: None,
        versions: Versions {
            probe: ProbeVersion::new("a".repeat(64)),
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
        "日志里有过一次进入终结集合 —— 后面写什么都改不了这件事"
    );
}

// ── 纠错的唯一出路：开新的一代 ─────────────────────────────────

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
            probe: Probe::new(
                Kind::new("shell"),
                serde_json::json!({ "run": "cat world.json" }),
            ),
            transitions: transitions(&[("obs.n > 100", r#"{ status: "settled" }"#)]),
            terminal: [StatusId::new("settled")].into_iter().collect(),
            initial: None,
            retain: Retain::Tick,
            cadence_secs: None,
            supersedes: Some(Supersede {
                key: key(),
                rationale: "阈值定错了，10 太低".as_bytes().to_vec(),
            }),
        })
        .await
        .unwrap();

    assert_eq!(opened.supersedes.as_ref(), Some(&key()));

    // 理由取得回来，跟 revise / close 的理由走同一条密封链。
    let cited = w.rt.read(&heir).await.unwrap().anchor.supersedes.unwrap();
    assert_eq!(cited.key, key());
    assert_eq!(
        w.rt.bindings().sealed(&cited.rationale).await.unwrap(),
        Some("阈值定错了，10 太低".as_bytes().to_vec()),
    );

    // 旧的那一代仍然是关的 —— 接替不是复活。
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
            probe: Probe::new(
                Kind::new("shell"),
                serde_json::json!({ "run": "cat world.json" }),
            ),
            transitions: Transitions::default(),
            terminal: Default::default(),
            initial: None,
            retain: Retain::Tick,
            cadence_secs: None,
            supersedes: Some(Supersede {
                key: key(),
                rationale: b"why".to_vec(),
            }),
        })
        .await
        .expect_err("两代同时活着说同一件事，就是绕过终结的旁路");
    assert!(e.to_string().contains("还开着"), "{e}");
}

// ── 锚可以先于它的目标存在 ─────────────────────────────────────

#[tokio::test]
async fn a_direction_that_has_not_grown_yet_warns_instead_of_refusing() {
    let w = World::new();
    w.write("{}");
    let opened =
        w.rt.open(OpenRequest {
            key: key(),
            probe: Probe::new(
                Kind::new("shell"),
                serde_json::json!({ "run": "cat world.json" }),
            ),
            transitions: transitions(&[(r#"changed("shape")"#, r#"{ shape: obs.shape }"#)]),
            terminal: Default::default(),
            initial: None,
            retain: Retain::Tick,
            cadence_secs: None,
            supersedes: None,
        })
        .await
        .expect("拼错和「还没长出来」在这一刻长得一样，都不该拦在开锚这里");

    assert!(
        opened.warnings.iter().any(|w| w.contains("no_such_field")),
        "但必须出声：{:?}",
        opened.warnings
    );

    // 长出来之后，它照常转换。
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
    let Observed::Attempt { reason, message } = &seen else {
        panic!("拼错的方向必须响，而不是恒假地静默下去：{seen:?}")
    };
    assert_eq!(*reason, gmr_core::ReasonClass::Unevaluable);
    assert!(message.contains("no_such_field"), "{message}");
}

use std::sync::Arc;

use gmr_core::{
    AnchorKey, Change, Expr, Kind, ProbeRef, Ref, Retain, Rule, State, StatusId, Transitions,
    Version,
};
use gmr_runtime::{Edge, OpenRequest, Policy, Runtime, Stall};
use gmr_store::testkit::{MemoryBindings, MemoryJournal, MemoryQueue};
use gmr_transport_shell::Shell;

/// 每个测试都发布一个真的 artifact —— 否则「版本是挣来的」这条只在
/// 生产路径上成立，测试反而绕过了它。
fn cat_probe(root: &std::path::Path) -> gmr_core::ProbeRef {
    let version =
        gmr_transport_shell::testkit::publish_script(root.join(".probes"), "cat world.json");
    gmr_core::ProbeRef::new(gmr_core::Kind::new("shell"), version, serde_json::json!({}))
}

struct World {
    dir: tempfile::TempDir,
    runtime: Runtime,
}

impl World {
    fn new() -> Self {
        Self::with(Policy::default(), false)
    }

    fn polled(policy: Policy) -> Self {
        Self::with(policy, true)
    }

    fn with(policy: Policy, queue: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut b = Runtime::builder()
            .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(Arc::new(MemoryBindings::default()))
            .policy(policy);
        if queue {
            b = b.queue(Arc::new(MemoryQueue::default()));
        }
        Self {
            dir,
            runtime: b.build(),
        }
    }

    fn write(&self, contents: &str) {
        std::fs::write(self.dir.path().join("world.json"), contents).unwrap();
    }

    #[allow(dead_code)]
    fn remove(&self) {
        let _ = std::fs::remove_file(self.dir.path().join("world.json"));
    }
}

fn key() -> AnchorKey {
    AnchorKey::new("a")
}

fn rules(pairs: &[(&str, &str)]) -> Transitions {
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

fn watching(direction: &str) -> Transitions {
    rules(&[(
        &format!("changed(\"{direction}\")"),
        &format!("{{ {direction}: obs.{direction}, status: \"drifted\" }}"),
    )])
}

fn request(root: &std::path::Path, transitions: Transitions) -> OpenRequest {
    OpenRequest {
        key: key(),
        probe: cat_probe(root),
        transitions,
        terminal: Default::default(),
        initial: None,
        retain: Retain::Tick,
        cadence_secs: None,
        supersedes: None,
    }
}

#[tokio::test]
async fn every_failure_path_emits_an_edge() {
    let w = World::polled(Policy {
        stalled_attempts: 2,
        stalled_blind_steps: 1,
        ..Default::default()
    });
    w.write(r#"{"shape":"old"}"#);
    w.runtime
        .open(request(w.dir.path(), watching("shape")))
        .await
        .unwrap();
    let start = w.runtime.changed_since(0, None).await.unwrap().cursor;

    w.write(r#"{"signature":"old"}"#);
    let o = w.runtime.observe(&key()).await.unwrap();
    assert!(
        matches!(&o, gmr_runtime::Observed::Attempt { reason, .. }
                 if *reason == gmr_core::ReasonClass::Unevaluable),
        "字段改名不能悄悄变成『没动静』：{o:?}"
    );

    let mid = w.runtime.changed_since(start, None).await.unwrap().cursor;
    for _ in 0..2 {
        w.runtime.observe(&key()).await.unwrap();
    }
    let e = w.runtime.changed_since(mid, None).await.unwrap();
    assert!(
        e.edges.iter().any(|x| matches!(
            x,
            Edge::Stalled {
                reason: Stall::Attempts { .. },
                ..
            }
        )),
        "连续算不出来也是停摆：{:?}",
        e.edges
    );

    w.write(r#"{"shape":"recovered"}"#);
    w.runtime.observe(&key()).await.unwrap();
    assert_eq!(
        w.runtime.read(&key()).await.unwrap().attempts,
        0,
        "看成一次就该清零"
    );

    let mid = w
        .runtime
        .changed_since(e.cursor, None)
        .await
        .unwrap()
        .cursor;
    w.runtime
        .revise(
            &key(),
            Change::Reprobe {
                probe: ProbeRef::new(
                    Kind::new("nonesuch"),
                    gmr_core::ProbeVersion::new("2".repeat(64)),
                    serde_json::json!({}),
                ),
            },
            b"switch to a transport nobody registered",
        )
        .await
        .unwrap();
    w.runtime.observe(&key()).await.unwrap();
    w.runtime.observe(&key()).await.unwrap();
    let e = w.runtime.changed_since(mid, None).await.unwrap();
    assert!(
        e.edges.iter().any(|x| matches!(
            x,
            Edge::Stalled {
                reason: Stall::Attempts { count: 2, .. },
                ..
            }
        )),
        "连续失败超阈值 → stalled：{:?}",
        e.edges
    );
}

#[tokio::test]
async fn a_cursor_makes_the_answer_incremental() {
    let w = World::new();
    w.write(r#"{"shape":"old"}"#);
    w.runtime
        .open(request(w.dir.path(), watching("shape")))
        .await
        .unwrap();

    let first = w.runtime.changed_since(0, None).await.unwrap();
    w.write(r#"{"shape":"new"}"#);
    w.runtime.observe(&key()).await.unwrap();

    let second = w.runtime.changed_since(first.cursor, None).await.unwrap();
    assert_eq!(second.edges.len(), 1);
    assert!(matches!(second.edges[0], Edge::Transitioned { .. }));

    let third = w.runtime.changed_since(second.cursor, None).await.unwrap();
    assert!(third.edges.is_empty(), "问过就不再重复");
}

#[tokio::test]
async fn edges_can_be_filtered_by_status() {
    let w = World::new();
    w.write(r#"{"shape":"a","body":"1"}"#);
    w.runtime
        .open(request(
            w.dir.path(),
            rules(&[
                (
                    "changed(\"shape\")",
                    r#"{ shape: obs.shape, body: obs.body, status: "shape-moved" }"#,
                ),
                (
                    "changed(\"body\")",
                    r#"{ shape: obs.shape, body: obs.body, status: "body-moved" }"#,
                ),
            ]),
        ))
        .await
        .unwrap();
    let start = w.runtime.changed_since(0, None).await.unwrap().cursor;

    w.write(r#"{"shape":"a","body":"2"}"#);
    w.runtime.observe(&key()).await.unwrap();

    assert!(
        w.runtime
            .changed_since(start, Some(&StatusId::new("shape-moved")))
            .await
            .unwrap()
            .edges
            .is_empty(),
        "签名没动，别烦我"
    );
    assert_eq!(
        w.runtime
            .changed_since(start, Some(&StatusId::new("body-moved")))
            .await
            .unwrap()
            .edges
            .len(),
        1
    );
}

#[tokio::test]
async fn entering_a_terminal_state_emits_both_edges() {
    let w = World::new();
    w.write(r#"{"done":false}"#);
    w.runtime
        .open(OpenRequest {
            terminal: [StatusId::new("done")].into_iter().collect(),
            ..request(
                w.dir.path(),
                rules(&[("obs.done == true", r#"{ status: "done" }"#)]),
            )
        })
        .await
        .unwrap();
    let start = w.runtime.changed_since(0, None).await.unwrap().cursor;

    w.write(r#"{"done":true}"#);
    w.runtime.observe(&key()).await.unwrap();

    let e = w.runtime.changed_since(start, None).await.unwrap();
    assert!(
        e.edges.iter().any(|x| matches!(
            x,
            Edge::Transitioned { status: Some(s), .. } if s.as_str() == "done"
        )),
        "说好的事有结果了"
    );
    assert!(
        e.edges.iter().any(|x| matches!(
            x,
            Edge::Closed {
                self_sealed: true,
                ..
            }
        )),
        "自封 —— 没有人写过理由"
    );
}

#[tokio::test]
async fn reading_the_previous_state_without_a_lease_only_warns() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    let opened = w
        .runtime
        .open(request(
            w.dir.path(),
            rules(&[("true", "{ n: state.n + 1 }")]),
        ))
        .await
        .unwrap();
    assert!(
        opened.warnings.iter().any(|s| s.contains("租约")),
        "{:?}",
        opened.warnings
    );

    let w = World::polled(Policy::default());
    w.write(r#"{"x":1}"#);
    let opened = w
        .runtime
        .open(request(
            w.dir.path(),
            rules(&[("true", "{ n: state.n + 1 }")]),
        ))
        .await
        .unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
}

#[tokio::test]
async fn a_pass_observes_due_anchors_and_reschedules_them() {
    let w = World::polled(Policy {
        cadence_secs: 1,
        ..Default::default()
    });
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();

    let p = w.runtime.pass().await.unwrap();
    assert_eq!(p.observed, 1);

    assert_eq!(w.runtime.pass().await.unwrap().observed, 0);
}

#[tokio::test]
async fn pass_without_a_queue_says_so() {
    let w = World::new();
    w.write("{}");
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    assert!(w.runtime.pass().await.is_err());
}

#[tokio::test]
async fn health_exposes_the_drift_quantities() {
    let w = World::new();
    w.write(r#"{"shape":"v1"}"#);
    w.runtime
        .open(request(w.dir.path(), watching("shape")))
        .await
        .unwrap();

    w.write(r#"{"shape":"v2"}"#);
    w.runtime.observe(&key()).await.unwrap();
    w.runtime
        .revise(
            &key(),
            Change::Restate {
                state: State::new(serde_json::json!({ "shape": "v2", "status": "ok" })),
            },
            b"accepted",
        )
        .await
        .unwrap();

    let h = w.runtime.health(&key()).await.unwrap();
    assert_eq!(h.restate_count, 1);
    assert!(
        h.state_drifted,
        "状态已经不是开锚那个了 —— 布尔，不是距离：基底不认识值的类型"
    );
    assert_eq!(h.rationale_sizes, vec![b"accepted".len()]);
    assert_eq!(h.stall_ratio, 0.0);
}

#[tokio::test]
async fn corpus_health_sees_barren_anchors() {
    let w = World::new();
    w.write("{}");
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();

    let c = w.runtime.corpus_health().await.unwrap();
    assert_eq!(c.active_anchors, 1);
    assert_eq!(c.barren_anchors, vec![key()]);
    assert_eq!(c.bound_refs, 0);

    w.runtime
        .bind(
            Ref::new("git", "m.md"),
            vec![key()],
            Version::new("v1"),
            vec![],
        )
        .await
        .unwrap();
    let c = w.runtime.corpus_health().await.unwrap();
    assert!(c.barren_anchors.is_empty());
    assert_eq!(c.memories_per_anchor.get("a"), Some(&1));
}

#[tokio::test]
async fn a_terminal_transition_reports_itself_as_self_sealed_exactly_once() {
    let w = World::new();
    w.write(r#"{"done":false}"#);
    w.runtime
        .open(OpenRequest {
            terminal: [gmr_core::StatusId::new("done")].into_iter().collect(),
            ..request(
                w.dir.path(),
                rules(&[("obs.done == true", r#"{ status: "done" }"#)]),
            )
        })
        .await
        .unwrap();

    w.write(r#"{"done":true}"#);
    w.runtime.observe(&key()).await.unwrap();

    let closed: Vec<bool> = w
        .runtime
        .changed_since(0, None)
        .await
        .unwrap()
        .edges
        .iter()
        .filter_map(|e| match e {
            gmr_runtime::Edge::Closed { self_sealed, .. } => Some(*self_sealed),
            _ => None,
        })
        .collect();

    assert_eq!(closed, vec![true], "自己走进终结集合 —— 一次，且是自封的");

    // 再问一次：已经交出去的边沿不重发。
    let again = w.runtime.changed_since(u64::MAX - 1, None).await.unwrap();
    assert!(
        !again
            .edges
            .iter()
            .any(|e| matches!(e, gmr_runtime::Edge::Closed { .. })),
        "游标之后没有新的关闭"
    );
}

#[tokio::test]
async fn an_author_close_is_not_reported_as_self_sealed() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime.close(&key(), b"collected").await.unwrap();

    let closed: Vec<bool> = w
        .runtime
        .changed_since(0, None)
        .await
        .unwrap()
        .edges
        .iter()
        .filter_map(|e| match e {
            gmr_runtime::Edge::Closed { self_sealed, .. } => Some(*self_sealed),
            _ => None,
        })
        .collect();
    assert_eq!(closed, vec![false], "作者伸手关的，处置跟自封不同");
}

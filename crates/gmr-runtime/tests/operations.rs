use std::sync::Arc;

use gmr_core::{
    AnchorKey, Change, Expr, Kind, ProbeRef, Ref, Retain, Rule, State, StatusId, Transitions,
    Version,
};
use gmr_runtime::{Edge, OpenRequest, Policy, Runtime};
use gmr_store::testkit::{MemoryBindings, MemoryJournal, MemoryQueue};
use gmr_transport_shell::Shell;

/// Every test publishes a real artifact. Otherwise "earned versions" would
/// hold on the production path while tests bypass it.
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
        let bindings = Arc::new(MemoryBindings::default());
        let mut b = Runtime::builder()
            .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(bindings.clone())
            .sealer(bindings)
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
        "a renamed field must not silently become no movement: {o:?}"
    );

    let mid = w.runtime.changed_since(start, None).await.unwrap().cursor;
    for _ in 0..2 {
        w.runtime.observe(&key()).await.unwrap();
    }
    let e = w.runtime.changed_since(mid, None).await.unwrap();
    assert!(
        e.edges
            .iter()
            .any(|x| matches!(x, Edge::Stalled { count: _, .. })),
        "consecutive unevaluable observations are stalled too: {:?}",
        e.edges
    );

    w.write(r#"{"shape":"recovered"}"#);
    w.runtime.observe(&key()).await.unwrap();
    assert_eq!(
        w.runtime.read(&key()).await.unwrap().attempts,
        0,
        "one successful observation should reset attempts"
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
        e.edges
            .iter()
            .any(|x| matches!(x, Edge::Stalled { count: 2, .. })),
        "consecutive failures above the threshold should become stalled: {:?}",
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
    assert!(
        third.edges.is_empty(),
        "already-read edges should not repeat"
    );
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
        "shape did not move, so do not report it"
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
        "the promised condition has resolved"
    );
    assert!(
        e.edges.iter().any(|x| matches!(
            x,
            Edge::Closed {
                self_sealed: true,
                ..
            }
        )),
        "self-sealed: no author rationale was written"
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
        opened.warnings.iter().any(|s| s.contains("no lease")),
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
        "state differs from the opening state; this is boolean, not a distance"
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

    assert_eq!(
        closed,
        vec![true],
        "entered the terminal set once and was self-sealed"
    );

    // Ask again: an already delivered edge is not emitted again.
    let again = w.runtime.changed_since(u64::MAX - 1, None).await.unwrap();
    assert!(
        !again
            .edges
            .iter()
            .any(|e| matches!(e, gmr_runtime::Edge::Closed { .. })),
        "there is no new close after the cursor"
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
    assert_eq!(
        closed,
        vec![false],
        "author close is distinct from self-sealing"
    );
}

#[tokio::test]
async fn an_event_is_handed_over_once_a_condition_is_reported_every_time() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.write(r#"{"x":2}"#);
    w.runtime.observe(&key()).await.unwrap();

    let first = w.runtime.changed_since(0, None).await.unwrap();
    assert!(
        !first.edges.is_empty(),
        "something did happen in the journal"
    );

    let second = w.runtime.changed_since(first.cursor, None).await.unwrap();
    assert!(
        second.edges.is_empty(),
        "already-read events are not delivered twice"
    );

    // Staleness is a standing condition, not a journal event. The cursor does
    // not apply, so it should be reported every time.
    let stale = World::polled(Policy {
        stalled_staleness_secs: -1,
        ..Default::default()
    });
    stale.write(r#"{"x":1}"#);
    stale
        .runtime
        .open(request(stale.dir.path(), watching("x")))
        .await
        .unwrap();

    let a = stale.runtime.changed_since(0, None).await.unwrap();
    let b = stale.runtime.changed_since(a.cursor, None).await.unwrap();
    assert_eq!(a.standing.len(), 1);
    assert_eq!(
        b.standing.len(),
        1,
        "there are no new entries after the cursor, but the anchor is still stale now"
    );
    assert!(
        b.edges.is_empty(),
        "and it must not pretend to be a new event"
    );
}

#[tokio::test]
async fn a_broken_rule_is_loud_on_the_first_failure_not_the_third() {
    let w = World::new();
    w.write(r#"{"here":1}"#);
    w.runtime
        .open(request(
            w.dir.path(),
            rules(&[("obs.gone > 1", r#"{ status: "x" }"#)]),
        ))
        .await
        .unwrap();

    let seen = w.runtime.observe(&key()).await.unwrap();
    assert!(
        matches!(
            seen,
            gmr_runtime::Observed::Attempt {
                reason: gmr_core::ReasonClass::Unevaluable,
                ..
            }
        ),
        "{seen:?}"
    );

    let stalled: Vec<_> = w
        .runtime
        .changed_since(0, None)
        .await
        .unwrap()
        .edges
        .into_iter()
        .filter(|e| matches!(e, Edge::Stalled { .. }))
        .collect();
    assert_eq!(
        stalled.len(),
        1,
        "a broken transition table will not improve by retrying; it should be loud on the first failure"
    );
    assert!(matches!(
        stalled[0],
        Edge::Stalled {
            last: gmr_core::ReasonClass::Unevaluable,
            count: 1,
            ..
        }
    ));
}

#[tokio::test]
async fn the_world_being_out_of_reach_still_waits_for_the_streak() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();

    // Unreachable is about the world, not a rule bug. It is worth retrying, so
    // it only becomes loud after the configured streak.
    std::fs::remove_file(w.dir.path().join("world.json")).unwrap();
    let seen = w.runtime.observe(&key()).await.unwrap();
    assert!(matches!(
        seen,
        gmr_runtime::Observed::Attempt {
            reason: gmr_core::ReasonClass::Unreachable,
            ..
        }
    ));

    assert!(
        !w.runtime
            .changed_since(0, None)
            .await
            .unwrap()
            .edges
            .iter()
            .any(|e| matches!(e, Edge::Stalled { .. })),
        "one unreachable observation is not stalled yet"
    );
}

#[tokio::test]
async fn a_hand_run_observation_takes_the_lease_instead_of_slipping_past_it() {
    let w = World::polled(Policy::default());
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();

    // Let polling write once; this anchor is now lease-managed.
    w.runtime.pass().await.unwrap();

    // Manual observation still works because it takes the lease instead of
    // bypassing the token.
    w.write(r#"{"x":2}"#);
    w.runtime.observe(&key()).await.unwrap();
    assert_eq!(
        w.runtime.read(&key()).await.unwrap().state.as_value()["x"],
        2
    );
}

#[tokio::test]
async fn an_observation_without_a_token_cannot_slip_in_beside_the_leaseholder() {
    use gmr_store::Fence;

    let w = World::polled(Policy::default());
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime.pass().await.unwrap();

    // Push an observation directly through the storage layer; this is the
    // second writer the lease exists to prevent.
    let entries = w.runtime.journal().entries(&key(), 0).await.unwrap();
    let (_, sighting) = entries
        .iter()
        .find(|(_, e)| e.is_sighting())
        .unwrap()
        .clone();
    let err = w
        .runtime
        .journal()
        .append(&key(), &sighting, Fence::Unleased)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "lease_managed_observation");
}

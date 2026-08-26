use std::sync::Arc;

use gmr_core::{
    AnchorKey, Change, Expr, Kind, ProbeRef, Ref, Retain, Rule, RunSettings, State, StatusId,
    Transitions, Version,
};
use gmr_runtime::{
    Bearing, Blind, Edge, Holding, Knowledge, Looked, Observed, OpenRequest, Policy, Runtime,
};
use gmr_store::BindingStore;
use gmr_store::testkit::{MemoryBindings, MemoryJournal, MemoryQueue};
use gmr_transport::shell::Shell;

fn cat_probe(root: &std::path::Path) -> gmr_core::ProbeRef {
    gmr_transport::shell::testkit::install_script(root.join(".probes"), "cat", "cat world.json")
}

#[derive(Default)]
struct Counted {
    inner: MemoryJournal,
    reads: std::sync::atomic::AtomicUsize,
}

impl Counted {
    fn reads(&self) -> usize {
        self.reads.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn reset(&self) {
        self.reads.store(0, std::sync::atomic::Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl gmr_store::Journal for Counted {
    async fn append(
        &self,
        anchor: &AnchorKey,
        entry: &gmr_core::Entry,
        fence: gmr_store::Fence,
    ) -> Result<gmr_core::Seq, gmr_store::StoreError> {
        self.inner.append(anchor, entry, fence).await
    }

    async fn entries(
        &self,
        anchor: &AnchorKey,
        from: gmr_core::Seq,
    ) -> Result<Vec<(gmr_core::Seq, gmr_core::Entry)>, gmr_store::StoreError> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.entries(anchor, from).await
    }

    async fn anchors(&self) -> Result<Vec<AnchorKey>, gmr_store::StoreError> {
        self.inner.anchors().await
    }

    async fn head(&self) -> Result<gmr_core::Seq, gmr_store::StoreError> {
        self.inner.head().await
    }
}

struct World {
    dir: tempfile::TempDir,
    runtime: Runtime,
    bindings: Arc<MemoryBindings>,
    queue: Arc<MemoryQueue>,
    journal: Arc<Counted>,
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
        let shared = Arc::new(MemoryQueue::default());
        let journal = Arc::new(Counted::default());
        let mut b = Runtime::builder()
            .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
            .journal(journal.clone())
            .bindings(bindings.clone())
            .sealer(bindings.clone())
            .links(bindings.clone())
            .settings(Arc::new(MemoryQueue::default()))
            .sightings(Arc::new(MemoryQueue::default()))
            .policy(policy);
        if queue {
            b = b.queue(shared.clone());
        }
        Self {
            dir,
            runtime: b.build(),
            bindings,
            queue: shared,
            journal,
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
        settings: RunSettings {
            facts: gmr_core::Recorded::Plain,
            budget_ms: None,
            retain: Retain::Tick,
            cadence_secs: None,
        },
        supersedes: None,
    }
}

#[tokio::test]
async fn one_look_reads_the_log_once_where_asking_twice_reads_it_twice() {
    let w = World::new();
    w.write(r#"{"shape":"old"}"#);
    w.runtime
        .open(request(w.dir.path(), watching("shape")))
        .await
        .unwrap();

    w.journal.reset();
    let read = w.runtime.read(&key()).await.unwrap();
    let observed = w.runtime.observe(&key()).await.unwrap();
    let apart = w.journal.reads();

    w.journal.reset();
    let Looked {
        before,
        observed: together,
    } = w.runtime.look(&key()).await.unwrap();
    let once = w.journal.reads();

    assert_eq!(
        apart, 2,
        "reading and then observing folds the same log twice — that is the cost being removed"
    );
    assert_eq!(
        once, 1,
        "a look already folded the log to observe from it; handing that state back must not \
         cost a second pass"
    );
    assert_eq!(
        before.state, read.state,
        "the view a look hands back is the one from before it looked, or `swapped` would \
         compare this build's instrument against itself and never report a swap"
    );
    assert_eq!(
        std::mem::discriminant(&together),
        std::mem::discriminant(&observed),
        "one call must answer what two answered"
    );
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
        e.edges.iter().any(|x| matches!(x, Edge::Stalled { .. })),
        "consecutive unevaluable observations are stalled too: {:?}",
        e.edges
    );

    w.write(r#"{"shape":"recovered"}"#);
    w.runtime.observe(&key()).await.unwrap();
    assert_eq!(
        w.runtime.read(&key()).await.unwrap().faltering,
        None,
        "one successful observation should clear the run of failures"
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
                    gmr_core::ProbeName::new("p"),
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
    assert!(
        w.runtime
            .changed_since(start, Some(&StatusId::new("body-moved")))
            .await
            .unwrap()
            .standing
            .is_none(),
        "a status filter is a specific question about edges; standing has no \
         status to filter by, so it must come back absent, not an empty list \
         that a caller cannot tell apart from 'nothing is currently stale'"
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

    let corpus = w.runtime.corpus().await.unwrap();
    let c = corpus.health();
    assert_eq!(c.active_anchors, 1);
    assert_eq!(c.barren_anchors, vec![key()]);
    assert_eq!(c.bound_refs, 0);

    w.runtime
        .bind(
            Ref::new("git", "m.md"),
            vec![key()],
            Some(Version::new("v1")),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();
    let corpus = w.runtime.corpus().await.unwrap();
    let c = corpus.health();
    assert!(c.barren_anchors.is_empty());
    assert_eq!(c.memories_per_anchor.get("a"), Some(&1));
    assert!(c.unsupervised.is_empty());
}

#[tokio::test]
async fn a_record_left_behind_by_the_anchor_that_watched_it_is_named() {
    let w = World::new();
    w.write("{}");
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    let note = Ref::new("git", "m.md");
    w.runtime
        .bind(
            note.clone(),
            vec![key()],
            Some(Version::new("v1")),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    let corpus = w.runtime.corpus().await.unwrap();
    assert!(corpus.health().unsupervised.is_empty());

    w.runtime
        .close(&key(), b"it served its purpose")
        .await
        .unwrap();

    let corpus = w.runtime.corpus().await.unwrap();
    assert_eq!(
        corpus.health().unsupervised,
        vec![note.clone()],
        "closing the last anchor a record hangs on is how a memory leaves the supervised \
         set, and it used to leave without a word: every corpus-level list filtered to the \
         open anchors first, so the record stopped being counted rather than being reported. \
         A note that still claims something about the code while nothing observes it is the \
         exact state this tool exists to make visible"
    );

    w.runtime
        .bind(
            note.clone(),
            vec![key()],
            Some(Version::new("v2")),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();
    assert_eq!(
        w.runtime.corpus().await.unwrap().health().unsupervised,
        vec![note.clone()],
        "one memory is named once however many assertions stand behind it. This list is \
         read as a roster of records, and a reference repeated once per assertion reads as \
         several memories in trouble where there is one"
    );
    assert!(
        corpus.health().barren_anchors.is_empty(),
        "the anchor is not barren — it has a record. It is the record that is stranded, and \
         conflating the two would report the wrong half of the pair"
    );
}

#[tokio::test]
async fn a_record_bound_to_an_anchor_nobody_ever_opened_is_stranded_too() {
    let w = World::new();
    w.write("{}");
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    let note = Ref::new("git", "orphan.md");
    w.runtime
        .bind(
            note.clone(),
            vec![AnchorKey::new("never-opened")],
            Some(Version::new("v1")),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    let corpus = w.runtime.corpus().await.unwrap();
    assert_eq!(
        corpus.health().unsupervised,
        vec![note],
        "`supervised` is one predicate — at least one anchor this record names is open — so \
         a key that closed and a key that never existed answer it the same way. Deriving the \
         list by walking anchors instead of records could only ever see the first"
    );
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
    assert_eq!(a.standing.iter().flatten().count(), 1);
    assert_eq!(
        b.standing.iter().flatten().count(),
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

    w.runtime.pass().await.unwrap();

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
    w.write(r#"{"x":2}"#);
    w.runtime.pass().await.unwrap();

    let entries = w.runtime.log().entries(&key(), 0).await.unwrap();
    let (_, sighting) = entries
        .iter()
        .rev()
        .find(|(_, e)| e.is_sighting())
        .unwrap()
        .clone();
    let err = w
        .runtime
        .log()
        .append(&key(), &sighting, Fence::Unleased)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "lease_managed_observation");
}

fn slow_probe(root: &std::path::Path, secs: &str) -> ProbeRef {
    gmr_transport::shell::testkit::install_script(
        root.join(".probes"),
        "slow",
        &format!("sleep {secs}; cat world.json"),
    )
}

async fn open_slow(w: &World, name: &str, secs: &str) -> AnchorKey {
    let key = AnchorKey::new(name);
    w.runtime
        .open(OpenRequest {
            key: key.clone(),
            probe: slow_probe(w.dir.path(), secs),
            transitions: watching("x"),
            terminal: Default::default(),
            initial: None,
            settings: RunSettings {
                facts: gmr_core::Recorded::Plain,
                budget_ms: None,
                retain: Retain::Tick,
                cadence_secs: None,
            },
            supersedes: None,
        })
        .await
        .unwrap();
    key
}

#[tokio::test]
async fn a_batch_that_runs_out_of_budget_does_not_blame_the_anchors_it_never_reached() {
    let w = World::polled(Policy {
        probe_budget_ms: 4000,
        cadence_secs: 300,
        ..Default::default()
    });
    w.write(r#"{"x":1}"#);

    let mut keys = Vec::new();
    for i in 0..30 {
        keys.push(open_slow(&w, &format!("a{i}"), "0.2").await);
    }

    let passed = w.runtime.pass().await.unwrap();

    let mut blamed = Vec::new();
    for key in &keys {
        if w.runtime.read(key).await.unwrap().faltering.is_some() {
            blamed.push(key.to_string());
        }
    }

    assert!(
        passed.observed < keys.len(),
        "the fixture is not exercising the cliff: every anchor fit inside the budget"
    );
    assert!(
        blamed.len() <= 1,
        "a batch that runs out of budget can produce at most one timed-out anchor — the one \
         in flight when the clock ran out. Everything behind it was never invoked, so it was \
         not observed and found wanting, it was not observed at all. Instead {} anchors carry \
         an attempt: {blamed:?}. An attempt backs the anchor off exponentially and, after {} \
         of them, reports it as stalled: 'this anchor is stuck' where the truth is 'we ran out \
         of time before its turn'",
        blamed.len(),
        Policy::default().stalled_attempts
    );
    assert_eq!(
        passed.observed + passed.skipped,
        keys.len(),
        "every due ticket is either observed or explicitly skipped. A pass that got through \
         half its batch has to say so, or it looks exactly like a pass that had nothing left \
         to do"
    );
}

#[tokio::test]
async fn an_anchor_the_budget_never_reached_comes_back_at_the_front_of_the_next_pass() {
    let w = World::polled(Policy {
        probe_budget_ms: 4000,
        cadence_secs: 3600,
        ..Default::default()
    });
    w.write(r#"{"x":1}"#);

    let mut keys = Vec::new();
    for i in 0..30 {
        keys.push(open_slow(&w, &format!("b{i}"), "0.2").await);
    }

    let first = w.runtime.pass().await.unwrap();
    assert!(first.skipped > 0);

    let again = w.runtime.pass().await.unwrap();
    assert!(
        again.observed > 0,
        "a skipped anchor is still due — it was never looked at. Rescheduling it a cadence \
         away would hide a starving tail behind a cadence of {}s, which is how a batch that \
         is permanently too small for its queue looks healthy forever",
        3600
    );
}

#[tokio::test]
async fn a_failed_observation_does_not_move_the_ground_under_a_memory() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime
        .bind(
            Ref::new("git", "m.md"),
            vec![key()],
            Some(Version::new("v1")),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    let bearing = async || {
        w.runtime.grounded(&key()).await.unwrap().memories[0]
            .warrant
            .as_ref()
            .expect("a bound memory always has a warrant")
            .bearing()
    };

    assert_eq!(
        bearing().await,
        Bearing::Holds,
        "nothing has happened since this was bound"
    );

    w.remove();
    let observed = w.runtime.observe(&key()).await.unwrap();
    assert!(
        matches!(observed, Observed::Attempt { .. }),
        "the probe cannot read a file that is not there, so this is our failure: {observed:?}"
    );
    assert_eq!(
        bearing().await,
        Bearing::Blind,
        "one failed look does not move the world. This compared the binding's seq against \
         the journal head, and the head advances on every entry -- including `Attempt`, \
         which records that we could not observe at all. Reported as moved, a memory reads \
         as standing on ground that shifted, when what actually happened is that nobody \
         could go and look"
    );

    w.write(r#"{"x":2}"#);
    w.runtime.observe(&key()).await.unwrap();
    assert_eq!(
        bearing().await,
        Bearing::Moved,
        "the world did move this time, and a comparison that never says so is worth less \
         than no comparison at all -- it reports every memory as standing on firm ground \
         forever"
    );
}

#[tokio::test]
async fn a_ground_that_moved_and_then_went_dark_says_both() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime
        .bind(
            Ref::new("git", "m.md"),
            vec![key()],
            Some(Version::new("v1")),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    w.write(r#"{"x":2}"#);
    w.runtime.observe(&key()).await.unwrap();
    w.remove();
    assert!(matches!(
        w.runtime.observe(&key()).await.unwrap(),
        Observed::Attempt { .. }
    ));

    let warrant = w.runtime.grounded(&key()).await.unwrap().memories[0]
        .warrant
        .clone()
        .expect("a bound memory always has a warrant");

    let Holding::Moved { ref axes, .. } = warrant.holding else {
        panic!(
            "we saw it move, and that stays established. Reporting only the outage would \
             throw away the one thing here we are certain of: {:?}",
            warrant.holding
        )
    };
    assert_eq!(
        axes,
        &["x".to_owned()],
        "the state paths that differ are named, so a reader can tell a memory about `x` \
         from one that never cared. An empty list here would make every move look alike"
    );
    assert!(
        matches!(
            warrant.knowledge,
            Knowledge::Blind {
                why: Blind::Unreachable { .. },
                ..
            }
        ),
        "and we cannot see it now, so we cannot say it has not moved again. Reporting only \
         the move would claim currency we do not have: {:?}",
        warrant.knowledge
    );
    assert_eq!(
        warrant.bearing(),
        Bearing::Moved,
        "flattened for counting, the established fact wins -- a reader chasing what moved \
         must still find it. The flat view is a tally, and the structured one is the answer"
    );
}

async fn open_named(w: &World, name: &str) -> AnchorKey {
    let key = AnchorKey::new(name);
    w.runtime
        .open(OpenRequest {
            key: key.clone(),
            probe: cat_probe(w.dir.path()),
            transitions: watching("x"),
            terminal: Default::default(),
            initial: None,
            settings: RunSettings {
                facts: gmr_core::Recorded::Plain,
                budget_ms: None,
                retain: Retain::Tick,
                cadence_secs: None,
            },
            supersedes: None,
        })
        .await
        .unwrap();
    key
}

#[tokio::test]
async fn a_memory_about_several_anchors_is_dated_against_each_of_them() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    let a = open_named(&w, "a").await;
    let b = open_named(&w, "b").await;

    let note = Ref::new("git", "many.md");
    w.runtime
        .bind(
            note.clone(),
            vec![a.clone(), b.clone()],
            Some(Version::new("v1")),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    let bearing_on = async |k: &AnchorKey| {
        w.runtime.grounded(k).await.unwrap().memories[0]
            .warrant
            .as_ref()
            .expect("a bound memory always has a warrant")
            .bearing()
    };

    assert_eq!(
        (bearing_on(&a).await, bearing_on(&b).await),
        (Bearing::Holds, Bearing::Holds),
        "a binding that names two anchors used to be stamped with no seq at all, on the \
         grounds that `which anchor's head would this be` has no answer. The journal's seq \
         is one global counter, so the question was the wrong one: a binding is dated \
         against the log, and one number does that for any number of anchors"
    );

    w.write(r#"{"x":2}"#);
    w.runtime.observe(&b).await.unwrap();

    assert_eq!(
        (bearing_on(&a).await, bearing_on(&b).await),
        (Bearing::Holds, Bearing::Moved),
        "one anchor moved and the other did not, and the same stamp has to tell them \
         apart. A stamp that reported both would hand back every memory in the corpus \
         whenever any one anchor moved"
    );
}

#[tokio::test]
async fn recapturing_a_world_that_did_not_move_leaves_the_memories_on_it_alone() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime
        .bind(
            Ref::new("git", "m.md"),
            vec![key()],
            Some(Version::new("v1")),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    let warrant = async || {
        w.runtime.grounded(&key()).await.unwrap().memories[0]
            .warrant
            .clone()
            .expect("a bound memory always has a warrant")
    };

    let before = w.runtime.read(&key()).await.unwrap().state;
    let blank = State::new(serde_json::json!({ "position": before.position() }));
    w.runtime
        .revise(
            &key(),
            Change::Restate { state: blank },
            b"the instrument moved",
        )
        .await
        .unwrap();
    w.runtime.observe(&key()).await.unwrap();

    assert_eq!(
        warrant().await.holding,
        Holding::Holds,
        "a recapture is the author re-pinning the anchor, not the world moving under it. \
         It restates and re-observes, so it advances `moved_at` while the state it lands \
         on is the state it left. Deciding by the seq alone reported `Moved` with an empty \
         axis list -- a claim contradicted by the very diff attached to it -- and one \
         `gmr rebase --all` after an extractor upgrade would have said that about every \
         dated note in the corpus at once"
    );
}

#[tokio::test]
async fn a_binding_that_carries_no_date_says_so_rather_than_claiming_no_ground() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();

    w.bindings
        .bind(&gmr_store::Asserted {
            binding: gmr_core::Binding {
                reference: Ref::new("git", "m.md"),
                anchors: vec![key()],
            },
            bound_version: Some(Version::new("v1")),
            bound_at_seq: None,
            source: gmr_core::Source::Adjudicated,
            at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    let warrant = w.runtime.grounded(&key()).await.unwrap().memories[0]
        .warrant
        .clone()
        .expect("a bound memory always has a warrant");

    assert_eq!(
        warrant.holding,
        Holding::Undated,
        "a row written before bindings carried a seq cannot be compared against the log \
         at all. Answering `NeverEstablished` said no ground was ever established, which \
         is false about a note that is bound and whose anchor is settled -- and it was the \
         answer for more than half of this repository's own notes"
    );
    assert_eq!(warrant.bearing(), Bearing::Undated);
}

#[tokio::test]
async fn a_reading_a_different_instrument_took_is_not_diffed_against_this_one() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime
        .bind(
            Ref::new("git", "m.md"),
            vec![key()],
            Some(Version::new("v1")),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    w.write(r#"{"x":2}"#);
    w.runtime.observe(&key()).await.unwrap();
    gmr_transport::shell::testkit::install_script(
        w.dir.path().join(".probes"),
        "cat",
        "cat ./world.json",
    );
    w.runtime.observe(&key()).await.unwrap();

    let holding = w.runtime.grounded(&key()).await.unwrap().memories[0]
        .warrant
        .clone()
        .expect("a bound memory always has a warrant")
        .holding;

    let Holding::Incomparable { took, reads } = holding else {
        panic!(
            "the state this memory was bound against was read by one extractor and the \
             state now by another. Diffing them answers `did the world move` with `the \
             instrument changed shape`: this repository's own corpus had 74 memories \
             reporting `Moved` on axes that were new keys in the state, with every body \
             hash identical. GMR.md's blast-radius clause asks the consumer to identify \
             that batch, not to absorb it: {holding:?}"
        )
    };
    assert_ne!(took, reads);
}

#[tokio::test]
async fn re_asserting_an_undated_binding_dates_it_instead_of_writing_nothing() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();

    let note = Ref::new("git", "m.md");
    w.bindings
        .bind(&gmr_store::Asserted {
            binding: gmr_core::Binding {
                reference: note.clone(),
                anchors: vec![key()],
            },
            bound_version: Some(Version::new("v1")),
            bound_at_seq: None,
            source: gmr_core::Source::Derived,
            at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    let landed = w
        .runtime
        .bind(
            note.clone(),
            vec![key()],
            Some(Version::new("v1")),
            gmr_core::Source::Derived,
        )
        .await
        .unwrap();
    assert!(
        landed.recorded,
        "`says` compared anchors, version and source, and a binding row also carries the \
         seq it was asserted at. Re-stating the claim over an undated row does add \
         something -- the date -- so answering `this already stands` left every row \
         written before the column permanently unanswerable, and nothing in the corpus \
         could ever heal itself"
    );

    let holding = w.runtime.grounded(&key()).await.unwrap().memories[0]
        .warrant
        .clone()
        .expect("a bound memory always has a warrant")
        .holding;
    assert_eq!(holding, Holding::Holds);

    let again = w
        .runtime
        .bind(
            note,
            vec![key()],
            Some(Version::new("v1")),
            gmr_core::Source::Derived,
        )
        .await
        .unwrap();
    assert!(
        !again.recorded,
        "and once dated it settles: the healing is one row per binding, not a row per run"
    );
}

#[tokio::test]
async fn an_anchor_that_records_digests_only_never_lets_a_plaintext_secret_into_the_log() {
    const SECRET: &str = "hunter2-the-actual-password";

    let w = World::new();
    w.write(&format!(r#"{{"x":"{SECRET}"}}"#));
    let mut opening = request(w.dir.path(), watching("x"));
    opening.settings.facts = gmr_core::Recorded::Digests;
    w.runtime.open(opening).await.unwrap_err();

    let entries = w.runtime.log().entries(&key(), 0).await.unwrap();
    assert!(
        entries.is_empty(),
        "an anchor that records digests only refuses the observation outright, so nothing \
         derived from the plaintext -- not the facts, not the state the rules built from \
         them -- is written. Replacing the facts with their hash on the way in would have \
         left the state, which the rules computed from the plaintext"
    );

    w.write(&format!(
        r#"{{"x":"{}"}}"#,
        "0000000000000000000000000000000000000000000000000000000000000000"
    ));
    let mut opening = request(w.dir.path(), watching("x"));
    opening.settings.facts = gmr_core::Recorded::Digests;
    w.runtime.open(opening).await.unwrap();

    let entries = w.runtime.log().entries(&key(), 0).await.unwrap();
    assert_eq!(entries.len(), 1, "a digest answers, so the anchor opens");

    let written = serde_json::to_string(&entries).unwrap();
    assert!(
        !written.contains(SECRET),
        "and the acceptance is mechanical rather than a promise: the secret is not \
         findable anywhere in what was written"
    );
}

#[tokio::test]
async fn refusing_an_undigested_reading_is_the_probes_to_fix_and_says_so() {
    let w = World::new();
    w.write(r#"{"x":"plaintext"}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime
        .set_settings(
            &key(),
            &RunSettings {
                retain: Retain::Tick,
                facts: gmr_core::Recorded::Digests,
                cadence_secs: None,
                budget_ms: None,
            },
        )
        .await
        .unwrap();

    let observed = w.runtime.observe(&key()).await.unwrap();
    let Observed::Attempt { reason, code, .. } = observed else {
        panic!("an undigested reading on a digests-only anchor is refused: {observed:?}")
    };
    assert_eq!(
        (reason, code),
        (
            gmr_core::ReasonClass::Unusable,
            gmr_core::FailureCode::Unusable,
        ),
        "the probe answered, and its answer cannot be used here. `Unreachable` would blame \
         the world for our own declaration, and `Unevaluable` would blame the rules, which \
         never ran"
    );
}

#[tokio::test]
async fn a_freshness_bound_decides_whether_to_look_again_not_what_to_report() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime
        .bind(
            Ref::new("git", "m.md"),
            vec![key()],
            Some(Version::new("v1")),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    w.write(r#"{"x":2}"#);

    let served = w.runtime.grounded(&key()).await.unwrap();
    assert_eq!(
        served.view.state.as_value()["x"],
        1,
        "with no freshness bound, grounding serves the reading it has and never goes out. \
         An instruction-free call is a default, not an absence of one"
    );

    let looked = w
        .runtime
        .grounded_within(
            &key(),
            &gmr_runtime::Instructions::fresher_than(std::time::Duration::ZERO),
        )
        .await
        .unwrap();
    assert_eq!(
        looked.view.state.as_value()["x"],
        2,
        "a bound of zero means anything already on record is too old, so this one goes and \
         looks. That is what makes staleness an observation instruction rather than a \
         verdict: it changes what GMR does, the way a budget does, instead of grading an \
         answer GMR had already settled on"
    );

    let again = w
        .runtime
        .grounded_within(
            &key(),
            &gmr_runtime::Instructions::fresher_than(std::time::Duration::from_secs(3600)),
        )
        .await
        .unwrap();
    assert_eq!(
        again.view.last_sighting, looked.view.last_sighting,
        "and an hour's slack over a reading taken a moment ago goes nowhere near the world"
    );
}

#[tokio::test]
async fn a_reading_that_could_not_be_refreshed_is_served_with_its_own_date_on_it() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime
        .bind(
            Ref::new("git", "m.md"),
            vec![key()],
            Some(Version::new("v1")),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    w.remove();
    let held = w
        .runtime
        .grounded_within(
            &key(),
            &gmr_runtime::Instructions::fresher_than(std::time::Duration::ZERO),
        )
        .await
        .unwrap();

    let warrant = held.memories[0]
        .warrant
        .clone()
        .expect("a bound memory always has a warrant");
    assert!(
        matches!(
            warrant.knowledge,
            Knowledge::Blind {
                why: Blind::Unreachable { .. },
                ..
            }
        ),
        "the refresh was asked for and failed, and the answer says so on the knowledge axis \
         rather than refusing the whole call. A freshness bound instructs; it does not \
         promise that the world will answer: {:?}",
        warrant.knowledge
    );
    assert_eq!(warrant.holding, Holding::Holds);
}

#[tokio::test]
async fn a_refresh_that_could_not_take_the_lease_says_so_rather_than_serving_stale_quietly() {
    use gmr_store::Queue;

    let w = World::polled(Policy::default());
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime.pass().await.unwrap();

    let held = w
        .queue
        .lease(&key(), chrono::Utc::now(), chrono::Duration::seconds(60))
        .await
        .unwrap();
    assert!(held.is_some(), "somebody else now holds this anchor");

    w.write(r#"{"x":2}"#);
    let asked = w
        .runtime
        .grounded_within(
            &key(),
            &gmr_runtime::Instructions::fresher_than(std::time::Duration::ZERO),
        )
        .await;

    assert!(
        matches!(asked, Err(gmr_runtime::RuntimeError::Leased { .. })),
        "the caller instructed a fresh reading and it could not be taken. Swallowing that \
         and serving the stored one left them to infer it from `observed_at` -- a failure \
         path with nothing on it, which is the one thing CLAUDE.md refuses. Who waits, \
         retries or accepts what is on record is the caller's call, and it cannot be made \
         from an answer that does not mention it: {asked:?}"
    );
}

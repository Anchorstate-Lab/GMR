use std::sync::{Arc, Mutex};

use gmr_core::{
    AnchorKey, Change, Expr, Kind, ProbeRef, Ref, Retain, Rule, RunSettings, State, StatusId,
    Transitions, Version,
};
use gmr_runtime::{
    Blind, Edge, Holding, HoldingKind, Knowledge, KnowledgeKind, Looked, Observed, OpenRequest,
    Policy, Runtime,
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
    rows: std::sync::atomic::AtomicUsize,
    stagger: std::sync::atomic::AtomicBool,
    contend: Mutex<Option<gmr_core::Entry>>,
}

impl Counted {
    fn reads(&self) -> usize {
        self.reads.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn stagger(&self) {
        self.stagger
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn rows(&self) -> usize {
        self.rows.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn reset(&self) {
        self.reads.store(0, std::sync::atomic::Ordering::SeqCst);
        self.rows.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    fn contend_once_with(&self, entry: gmr_core::Entry) {
        *self.contend.lock().unwrap() = Some(entry);
    }
}

#[async_trait::async_trait]
impl gmr_store::Journal for Counted {
    async fn append(
        &self,
        anchor: &AnchorKey,
        entry: &gmr_core::Entry,
        fence: gmr_store::Fence,
        expected: gmr_store::Expected,
    ) -> Result<gmr_core::Seq, gmr_store::StoreError> {
        let ahead = self.contend.lock().unwrap().take();
        if let Some(ahead) = ahead {
            self.inner
                .append(anchor, &ahead, fence, gmr_store::Expected::Any)
                .await?;
        }
        self.inner.append(anchor, entry, fence, expected).await
    }

    async fn entries(
        &self,
        anchor: &AnchorKey,
        from: gmr_core::Seq,
    ) -> Result<Vec<(gmr_core::Seq, gmr_core::Entry)>, gmr_store::StoreError> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.stagger.load(std::sync::atomic::Ordering::SeqCst) {
            let first = anchor.as_str().bytes().next().unwrap_or(b'a');
            let ms = u64::from(b'z'.saturating_sub(first)) * 4;
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        }
        let got = self.inner.entries(anchor, from).await;
        if let Ok(rows) = &got {
            self.rows
                .fetch_add(rows.len(), std::sync::atomic::Ordering::SeqCst);
        }
        got
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

    fn elsewhere(&self) -> World {
        let dir = tempfile::tempdir().unwrap();
        let bindings = self.bindings.clone();
        let runtime = Runtime::builder()
            .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
            .journal(self.journal.clone())
            .bindings(bindings.clone())
            .sealer(bindings.clone())
            .links(bindings.clone())
            .settings(Arc::new(MemoryQueue::default()))
            .sightings(Arc::new(MemoryQueue::default()))
            .policy(Policy::default())
            .build();
        World {
            dir,
            runtime,
            bindings,
            queue: Arc::new(MemoryQueue::default()),
            journal: self.journal.clone(),
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
            .raised
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
            gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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
            gmr_core::Binding::on(note.clone(), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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
            gmr_core::Binding::on(note.clone(), vec![key()]),
            Some(Version::new("v2")),
            Default::default(),
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
            gmr_core::Binding::on(note.clone(), vec![AnchorKey::new("never-opened")]),
            Some(Version::new("v1")),
            Default::default(),
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
    assert_eq!(a.raised.iter().flatten().count(), 1);
    assert_eq!(
        b.raised.iter().flatten().count(),
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
async fn an_entry_folded_against_a_head_that_moved_is_refused() {
    use gmr_store::{Expected, Fence};

    let w = World::polled(Policy::default());
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.write(r#"{"x":2}"#);
    w.runtime.pass().await.unwrap();

    let entries = w.runtime.log().entries(&key(), 0).await.unwrap();
    let (at, sighting) = entries
        .iter()
        .rev()
        .find(|(_, e)| e.is_sighting())
        .unwrap()
        .clone();
    let head = entries.last().unwrap().0;

    let err = w
        .runtime
        .log()
        .append(&key(), &sighting, Fence::Unleased, Expected::Head(at - 1))
        .await
        .unwrap_err();
    assert_eq!(
        err.code(),
        "head_moved",
        "an observation folded against a state that has since been written past is not \
         admitted just because the writer holds no lease -- the lease was never what made \
         it safe"
    );
    assert!(err.head_moved());

    w.runtime
        .log()
        .append(&key(), &sighting, Fence::Unleased, Expected::Head(head))
        .await
        .expect("folded against the head that is actually there, it goes in unleased");
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
            gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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

    let settled = warrant().await;
    assert_eq!(
        (settled.holding.kind(), settled.knowledge.kind()),
        (HoldingKind::Holds, KnowledgeKind::Seen),
        "nothing has happened since this was bound, and we looked"
    );

    w.remove();
    let observed = w.runtime.observe(&key()).await.unwrap();
    assert!(
        matches!(observed, Observed::Attempt { .. }),
        "the probe cannot read a file that is not there, so this is our failure: {observed:?}"
    );
    let blinded = warrant().await;
    assert_eq!(
        blinded.holding.kind(),
        HoldingKind::Holds,
        "one failed look does not move the world. This compared the binding's seq against \
         the journal head, and the head advances on every entry -- including `Attempt`, \
         which records that we could not observe at all. Reported as moved, a memory reads \
         as standing on ground that shifted, when what actually happened is that nobody \
         could go and look"
    );
    assert!(
        matches!(blinded.knowledge, Knowledge::Blind { .. }),
        "and the failure is said on the axis that owns it, not by degrading the other one: \
         {:?}",
        blinded.knowledge
    );

    w.write(r#"{"x":2}"#);
    w.runtime.observe(&key()).await.unwrap();
    assert_eq!(
        warrant().await.holding.kind(),
        HoldingKind::Moved,
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
            gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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
            gmr_core::Binding::on(note.clone(), vec![a.clone(), b.clone()]),
            Some(Version::new("v1")),
            Default::default(),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    let holding_on = async |k: &AnchorKey| {
        w.runtime.grounded(k).await.unwrap().memories[0]
            .warrant
            .as_ref()
            .expect("a bound memory always has a warrant")
            .holding
            .kind()
    };

    assert_eq!(
        (holding_on(&a).await, holding_on(&b).await),
        (HoldingKind::Holds, HoldingKind::Holds),
        "a binding that names two anchors used to be stamped with no seq at all, on the \
         grounds that `which anchor's head would this be` has no answer. The journal's seq \
         is one global counter, so the question was the wrong one: a binding is dated \
         against the log, and one number does that for any number of anchors"
    );

    w.write(r#"{"x":2}"#);
    w.runtime.observe(&b).await.unwrap();

    assert_eq!(
        (holding_on(&a).await, holding_on(&b).await),
        (HoldingKind::Holds, HoldingKind::Moved),
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
            gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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
            binding: gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            bound_version: Some(Version::new("v1")),
            bound_at_seq: None,
            saw: Default::default(),
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
}

#[tokio::test]
async fn a_key_only_the_newer_instrument_measures_is_not_the_older_one_disagreeing() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(
            w.dir.path(),
            rules(&[("true", r#"{ v: obs.x, status: "s" }"#)]),
        ))
        .await
        .unwrap();
    w.runtime
        .bind(
            gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    gmr_transport::shell::testkit::install_script(
        w.dir.path().join(".probes"),
        "cat",
        "cat ./world.json",
    );
    w.runtime
        .revise(
            &key(),
            Change::Retransition {
                transitions: rules(&[("true", r#"{ v: obs.x, w: obs.x, status: "s" }"#)]),
            },
            b"the newer instrument measures one more thing",
        )
        .await
        .unwrap();
    w.runtime.observe(&key()).await.unwrap();

    let holding = w.runtime.grounded(&key()).await.unwrap().memories[0]
        .warrant
        .clone()
        .expect("a bound memory always has a warrant")
        .holding;

    assert_eq!(
        holding,
        Holding::Holds,
        "`v` is unchanged and `w` is a path the older reading never carried. Silence is \
         not disagreement: the old instrument did not contradict `w`, it never measured \
         it. Counting it made 66 of this repository's own notes unanswerable after an \
         extractor upgrade that changed nothing they were about"
    );
}

#[tokio::test]
async fn a_path_the_newer_instrument_stopped_measuring_still_cannot_be_compared() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(
            w.dir.path(),
            rules(&[("true", r#"{ v: obs.x, w: obs.x, status: "s" }"#)]),
        ))
        .await
        .unwrap();
    w.runtime
        .bind(
            gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    gmr_transport::shell::testkit::install_script(
        w.dir.path().join(".probes"),
        "cat",
        "cat ./world.json",
    );
    w.runtime
        .revise(
            &key(),
            Change::Retransition {
                transitions: rules(&[("true", r#"{ v: obs.x, status: "s" }"#)]),
            },
            b"the newer instrument stopped measuring one thing",
        )
        .await
        .unwrap();
    w.runtime.observe(&key()).await.unwrap();

    let holding = w.runtime.grounded(&key()).await.unwrap().memories[0]
        .warrant
        .clone()
        .expect("a bound memory always has a warrant")
        .holding;

    assert!(
        matches!(holding, Holding::Incomparable { .. }),
        "a path that vanished is not silence, it is an instrument that stopped looking, \
         and nothing here can say whether what it used to measure moved. Keeping removals \
         is also what stops a renamed key from reading as `Holds`: a rename arrives as an \
         addition and a removal, and the removal is the half that refuses: {holding:?}"
    );
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
            gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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
            binding: gmr_core::Binding::on(note.clone(), vec![key()]),
            bound_version: Some(Version::new("v1")),
            bound_at_seq: None,
            saw: Default::default(),
            source: gmr_core::Source::Derived,
            at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    let landed = w
        .runtime
        .bind(
            gmr_core::Binding::on(note.clone(), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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
            gmr_core::Binding::on(note, vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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
            gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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
            gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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

fn at(k: &str, root: &std::path::Path, transitions: Transitions) -> OpenRequest {
    OpenRequest {
        key: AnchorKey::new(k),
        ..request(root, transitions)
    }
}

#[tokio::test]
async fn an_unchanged_reading_appends_nothing_and_leaves_the_warrant_where_it_was() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime
        .bind(
            gmr_core::Binding::on(Ref::new("git", "m.md"), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
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
    let before = warrant().await;
    let head = w.runtime.log().head().await.unwrap();

    for _ in 0..3 {
        assert_eq!(
            w.runtime.observe(&key()).await.unwrap(),
            Observed::Still,
            "the facts did not move and neither did the instrument, so re-reading them is \
             not news"
        );
    }

    assert_eq!(
        w.runtime.log().head().await.unwrap(),
        head,
        "three re-observations of an unmoved fact must append nothing. Every entry is a \
         reason to wake whoever depends on this anchor, so a log that grows on each poll \
         is a product that pages a human for the world staying still"
    );
    let after = warrant().await;
    assert_eq!(
        after.holding, before.holding,
        "and the axis that would wake anybody did not move either -- an early cutoff that \
         still perturbs `holding` has only moved the firehose downstream"
    );

    let (Knowledge::Seen { at: then, .. }, Knowledge::Seen { at: now, .. }) =
        (&before.knowledge, &after.knowledge)
    else {
        panic!("we looked, and looking is what `Seen` records: {after:?}")
    };
    assert!(
        now > then,
        "the other axis must move, and this is the whole reason there are two. Looking \
         again at a fact that did not move is not news about the fact -- but it is news \
         about us: we know it more recently than we did. That is recorded as a sighting on \
         the scheduler rather than an entry in the journal, so freshness improves without \
         costing anyone a wake-up. Collapse the axes and this becomes unsayable: either \
         every poll appends, or a caller asking for a fact fresher than an hour is told to \
         re-probe something that was read a second ago"
    );
}

#[tokio::test]
async fn the_same_record_buckets_under_two_holdings_because_it_hangs_on_two_anchors() {
    let w = World::new();
    w.write(r#"{"x":1,"y":1}"#);
    w.runtime
        .open(at("moves", w.dir.path(), watching("x")))
        .await
        .unwrap();
    w.runtime
        .open(at("stays", w.dir.path(), watching("y")))
        .await
        .unwrap();

    let reference = Ref::new("git", "m.md");
    w.runtime
        .bind(
            gmr_core::Binding::on(
                reference.clone(),
                vec![AnchorKey::new("moves"), AnchorKey::new("stays")],
            ),
            Some(Version::new("v1")),
            Default::default(),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    w.write(r#"{"x":2,"y":1}"#);
    w.runtime.observe(&AnchorKey::new("moves")).await.unwrap();
    w.runtime.observe(&AnchorKey::new("stays")).await.unwrap();

    let corpus = w.runtime.corpus().await.unwrap();
    let holdings = &corpus.health().holdings;

    assert_eq!(
        holdings.get(&HoldingKind::Moved).map(|a| a
            .iter()
            .map(|(k, r)| (k.to_string(), r.clone()))
            .collect::<Vec<_>>()),
        Some(vec![("moves".to_owned(), vec![reference.clone()])]),
        "the ground under `moves` shifted, and the tally must say which anchor that was"
    );
    assert_eq!(
        holdings.get(&HoldingKind::Holds).map(|a| a
            .iter()
            .map(|(k, r)| (k.to_string(), r.clone()))
            .collect::<Vec<_>>()),
        Some(vec![("stays".to_owned(), vec![reference.clone()])]),
        "the same record is still standing on `stays`, so it is in both buckets at once"
    );

    assert_eq!(
        holdings
            .values()
            .flat_map(|a| a.values())
            .flatten()
            .filter(|r| **r == reference)
            .count(),
        2,
        "one record, two anchors, two answers. This is why the tally is two levels deep: \
         flattened to BTreeMap<HoldingKind, Vec<Ref>> the reader is told this note both \
         moved and holds and cannot find out which anchor said which, which is the one \
         thing they need in order to go and look"
    );
}

#[tokio::test]
async fn folding_the_same_log_again_reads_only_what_was_appended_since() {
    let w = World::new();
    w.write(r#"{"x":0}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();
    for n in 1..=8 {
        w.write(&format!(r#"{{"x":{n}}}"#));
        w.runtime.observe(&key()).await.unwrap();
    }

    w.journal.reset();
    let caught_up = w.runtime.read(&key()).await.unwrap();
    assert_eq!(
        w.journal.rows(),
        1,
        "observing folds *before* it appends, so the checkpoint trails the last append by \
         exactly one entry and reading it back costs that one. `append` deliberately does \
         not fold onto the checkpoint: extension is the only operation this cache has, and \
         giving it a second writer is how an invalidation bug gets in"
    );

    w.journal.reset();
    let warm = w.runtime.read(&key()).await.unwrap();
    assert_eq!(
        w.journal.rows(),
        0,
        "and with nothing appended since, the next fold reads nothing at all. This is the \
         cost D-5 was about: this repository's own journal is 47 MB across 60k entries, \
         and `check` was deserializing all of it twice per run to learn what it already knew"
    );
    assert_eq!(
        warm.state, caught_up.state,
        "reading twice cannot change the answer"
    );

    let cold = w.elsewhere();
    cold.journal.reset();
    let fresh = cold.runtime.read(&key()).await.unwrap();
    assert!(
        cold.journal.rows() >= 9,
        "a runtime that has never folded this log has no checkpoint to stand on and reads \
         all of it: {}",
        cold.journal.rows()
    );
    assert_eq!(
        fresh.state, warm.state,
        "and it lands byte for byte where the warm one did. Two readers holding \
         checkpoints of different ages must not be able to disagree -- that is what makes \
         this safe to keep per-process and safe to leave with no invalidation at all"
    );

    w.write(r#"{"x":99"#);
    let _ = w.runtime.observe(&key()).await;
    w.write(r#"{"x":99}"#);
    w.runtime.observe(&key()).await.unwrap();

    w.journal.reset();
    let moved = w.runtime.read(&key()).await.unwrap();
    assert!(
        w.journal.rows() <= 3,
        "a log that grew by a failed look and a move costs those entries to catch up on, \
         not the whole log: {}",
        w.journal.rows()
    );
    assert_eq!(
        moved.state.0.get("x").and_then(|v| v.as_i64()),
        Some(99),
        "the caught-up state is the real one, not a stale checkpoint handed back"
    );
}

#[tokio::test]
async fn looking_at_many_at_once_answers_in_the_order_asked() {
    let w = World::new();
    w.write(r#"{"x":1,"y":1,"z":1}"#);
    let keys: Vec<AnchorKey> = ["c", "a", "b", "e", "d"]
        .iter()
        .map(|k| AnchorKey::new(*k))
        .collect();
    for (k, axis) in keys.iter().zip(["x", "y", "z", "x", "y"]) {
        w.runtime
            .open(at(k.as_str(), w.dir.path(), watching(axis)))
            .await
            .unwrap();
    }

    w.journal.stagger();
    let together = w.runtime.look_all(&keys).await.unwrap();
    assert_eq!(
        together
            .iter()
            .map(|l| l.before.key.clone())
            .collect::<Vec<_>>(),
        keys,
        "concurrency must not reorder the answers. `check` zips these against the keys it \
         asked about and prints one line per pair, so an answer arriving out of order would \
         attribute one anchor's drift to another -- silently, and worse the more anchors \
         there are. `buffered` keeps the order; `buffer_unordered` would not"
    );

    let mut serially = Vec::new();
    for key in &keys {
        serially.push(w.runtime.look(key).await.unwrap());
    }
    assert_eq!(
        together
            .iter()
            .map(|l| (l.before.key.clone(), l.before.state.clone()))
            .collect::<Vec<_>>(),
        serially
            .iter()
            .map(|l| (l.before.key.clone(), l.before.state.clone()))
            .collect::<Vec<_>>(),
        "and observing five anchors at once must land where observing them one at a time \
         lands. Each holds its own fence and appends only under its own key, so there is \
         nothing for them to race over -- this is the assertion that keeps that true if \
         anything shared ever creeps in"
    );
}

#[tokio::test]
async fn a_batch_of_one_and_a_batch_of_none_are_both_answerable() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();

    assert!(
        w.runtime.look_all(&[]).await.unwrap().is_empty(),
        "no anchors is an empty answer, not an error -- `check` on a fresh repository asks \
         exactly this"
    );
    assert_eq!(
        w.runtime.look_all(&[key()]).await.unwrap().len(),
        1,
        "and one anchor still answers once, whatever the concurrency bound is"
    );
    assert!(
        w.runtime
            .look_all(&[AnchorKey::new("never-opened")])
            .await
            .is_err(),
        "an anchor nobody opened is still the error it was one at a time. A batch must not \
         quietly drop the member it could not answer for"
    );
}

fn folded_state(entries: &[(gmr_core::Seq, gmr_core::Entry)]) -> State {
    gmr_core::fold(entries).unwrap().state
}

#[tokio::test]
async fn no_entry_is_ever_folded_from_a_state_something_else_already_replaced() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();

    let stolen = {
        let entries = w.runtime.log().entries(&key(), 0).await.unwrap();
        let (_, opened) = entries.last().unwrap().clone();
        let gmr_core::Entry::Open {
            observation, at, ..
        } = opened
        else {
            unreachable!("opening appends an Open")
        };
        gmr_core::Entry::Transition {
            observation,
            state: State::new(serde_json::json!({ "x": 7, "status": "drifted" })),
            at,
        }
    };

    w.write(r#"{"x":2}"#);
    w.journal.contend_once_with(stolen);
    w.runtime.observe(&key()).await.unwrap();

    let entries = w.runtime.log().entries(&key(), 0).await.unwrap();
    assert_eq!(
        entries.len(),
        3,
        "the open, the entry that got in first, and the replayed one -- the observation is \
         not thrown away and it is not written twice"
    );

    for cut in 1..entries.len() {
        let (seq, entry) = &entries[cut];
        let gmr_core::Entry::Transition { state, .. } = entry else {
            continue;
        };
        let before = folded_state(&entries[..cut]);
        let after = folded_state(&entries[..=cut]);
        assert_eq!(
            &after, state,
            "entry {seq} claims a state the fold does not land on"
        );
        assert_ne!(
            before, *state,
            "entry {seq} was folded against a state something else had already replaced \
             -- this is the lost update the head expectation exists to refuse"
        );
    }

    assert_eq!(
        w.runtime.read(&key()).await.unwrap().state.as_value()["x"],
        2,
        "the replay recomputed against what the other writer left, so the reading we \
         actually took is the one standing"
    );
}

#[tokio::test]
async fn a_replay_does_not_put_out_a_bit_the_other_writer_just_lit() {
    let accumulating = rules(&[(
        "true",
        r#"{ x: obs.x, lit: state.lit or (obs.y != 0), status: "seen" }"#,
    )]);

    let w = World::new();
    w.write(r#"{"x":1,"y":0}"#);
    let mut opening = request(w.dir.path(), accumulating);
    opening.initial = Some(State::new(
        serde_json::json!({ "x": 1, "lit": false, "status": "seen" }),
    ));
    w.runtime.open(opening).await.unwrap();
    assert_eq!(
        w.runtime.read(&key()).await.unwrap().state.as_value()["lit"],
        serde_json::json!(false),
        "opening starts the accumulation at zero"
    );

    let lit = {
        let entries = w.runtime.log().entries(&key(), 0).await.unwrap();
        let (_, opened) = entries.last().unwrap().clone();
        let gmr_core::Entry::Open {
            observation, at, ..
        } = opened
        else {
            unreachable!("opening appends an Open")
        };
        gmr_core::Entry::Transition {
            observation,
            state: State::new(serde_json::json!({ "x": 1, "lit": true, "status": "seen" })),
            at,
        }
    };

    w.write(r#"{"x":2,"y":0}"#);
    w.journal.contend_once_with(lit);
    w.runtime.observe(&key()).await.unwrap();

    let state = w.runtime.read(&key()).await.unwrap().state;
    assert_eq!(
        state.as_value()["x"],
        serde_json::json!(2),
        "our own reading is what stands on the axis we actually measured"
    );
    assert_eq!(
        state.as_value()["lit"],
        serde_json::json!(true),
        "our observation reads `y` unchanged, so nothing we saw lights this bit -- it is \
         true only because the replay recomputed against the state the other writer left, \
         and an accumulating axis reads its own last bit. Drop the `state.lit or` and \
         whoever observes second eats a drift signal nobody will ever see again"
    );
}

#[tokio::test]
async fn two_opens_of_one_key_cannot_both_land() {
    let w = World::new();
    w.write(r#"{"x":1}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();

    let second = w.elsewhere();
    second.write(r#"{"x":1}"#);
    let err = second
        .runtime
        .open(request(second.dir.path(), watching("x")))
        .await
        .unwrap_err();

    assert!(
        matches!(err, gmr_runtime::RuntimeError::AlreadyOpen { .. }),
        "got {err:?}"
    );
    assert_eq!(
        w.runtime.log().entries(&key(), 0).await.unwrap().len(),
        1,
        "a second Open replaces the fold outright, so it does not add a duplicate entry -- \
         it silently discards every observation and every accumulated bit since the first one"
    );
}

#[tokio::test]
async fn grounding_reads_the_whole_log_only_when_the_binding_predates_the_move() {
    let w = World::new();
    w.write(r#"{"x":0}"#);
    w.runtime
        .open(request(w.dir.path(), watching("x")))
        .await
        .unwrap();

    let early: gmr_core::Claim = Ref::new("git", "early.md").into();
    w.runtime
        .bind(
            gmr_core::Binding::on(early.clone(), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    for n in 1..=8 {
        w.write(&format!(r#"{{"x":{n}}}"#));
        w.runtime.observe(&key()).await.unwrap();
    }

    let late: gmr_core::Claim = Ref::new("git", "late.md").into();
    w.runtime
        .bind(
            gmr_core::Binding::on(late.clone(), vec![key()]),
            Some(Version::new("v1")),
            Default::default(),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    let how = gmr_runtime::Instructions::default();

    w.journal.reset();
    w.runtime
        .ground(std::slice::from_ref(&late), &how)
        .await
        .unwrap();
    assert_eq!(
        w.journal.rows(),
        0,
        "bound after the last move, so the answer is `Holds` and nothing has to be folded \
         back to. The view itself comes off the checkpoint"
    );

    w.journal.reset();
    let held = w
        .runtime
        .ground(std::slice::from_ref(&early), &how)
        .await
        .unwrap();
    assert!(
        w.journal.rows() >= 9,
        "bound before the moves, so `Holding` has to fold the log back to the moment of \
         binding to say what changed. That read is the one thing here that genuinely needs \
         the whole log, and it is asked for only in this case: {}",
        w.journal.rows()
    );
    assert!(
        matches!(held[0].on[0].warrant(), Some(w) if !matches!(w.holding, Holding::Holds)),
        "{:?}",
        held[0].on[0]
    );

    let cold = w.elsewhere();
    cold.journal.reset();
    cold.runtime.ground(&[late], &how).await.unwrap();
    assert!(
        cold.journal.rows() >= 9,
        "a runtime that has never folded this log has no checkpoint to stand on, which is \
         what makes the zero above a measurement and not an accident: {}",
        cold.journal.rows()
    );

    w.journal.reset();
    w.runtime.ground(&[early], &how).await.unwrap();
    assert!(
        w.journal.rows() >= 9,
        "and the fold-back is not cached either"
    );
}

fn reading_the_file(
    dir: &std::path::Path,
) -> (Arc<gmr_transport::file::Files>, gmr_core::ProbeRef) {
    let asks = std::collections::BTreeMap::from([(
        gmr_core::ProbeName::new("world"),
        gmr_transport::file::Ask::at("world.json"),
    )]);
    (
        Arc::new(gmr_transport::file::Files::new(dir, asks)),
        gmr_core::ProbeRef::new(
            gmr_core::Kind::new("file"),
            gmr_core::ProbeName::new("world"),
            serde_json::Value::Null,
        ),
    )
}

#[tokio::test]
async fn an_anchor_whose_rules_read_what_its_probe_never_reports_is_refused_at_open() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("world.json"), r#"{"x":0}"#).unwrap();
    let (files, probe) = reading_the_file(dir.path());
    let rt = Runtime::builder()
        .transport(files)
        .journal(Arc::new(MemoryJournal::default()))
        .bindings(Arc::new(MemoryBindings::default()))
        .sealer(Arc::new(MemoryBindings::default()))
        .links(Arc::new(MemoryBindings::default()))
        .settings(Arc::new(MemoryQueue::default()))
        .sightings(Arc::new(MemoryQueue::default()))
        .build();

    let key = gmr_core::AnchorKey::new("blind");
    let asked = |transitions| gmr_runtime::OpenRequest {
        key: key.clone(),
        probe: probe.clone(),
        transitions,
        terminal: Default::default(),
        initial: None,
        settings: Default::default(),
        supersedes: None,
    };

    let refused = rt
        .open(asked(gmr_core::Transitions(vec![gmr_core::Rule {
            when: gmr_core::Expr::text("obs.price_cents != state.price"),
            to: gmr_core::Expr::text("{ price: obs.price_cents }"),
        }])))
        .await;
    let Err(gmr_runtime::RuntimeError::CannotOpen { message }) = refused else {
        panic!("{refused:?}")
    };
    assert!(message.contains("obs.price_cents"), "{message}");
    assert!(
        message.contains("obs.value"),
        "naming what the probe does report is what turns a refusal into a fix: {message}"
    );

    assert!(
        rt.log().entries(&key, 0).await.unwrap().is_empty(),
        "a refused open writes nothing. An anchor that exists and can never transition is \
         worse than one that was never opened, because it reads as supervised"
    );

    assert!(
        rt.open(asked(gmr_core::Transitions(vec![gmr_core::Rule {
            when: gmr_core::Expr::text("obs.value.x != state.x"),
            to: gmr_core::Expr::text("{ x: obs.value.x }"),
        }])))
        .await
        .is_ok(),
        "the same key opens once the rules read where the probe actually looks"
    );
}

#[tokio::test]
async fn a_probe_that_cannot_say_what_it_reports_does_not_refuse_anything() {
    let w = World::new();
    w.write(r#"{"x":0}"#);
    let script = gmr_transport::shell::testkit::install_script(
        w.dir.path().join(".probes"),
        "anything",
        "cat world.json",
    );
    let asked = gmr_runtime::OpenRequest {
        key: gmr_core::AnchorKey::new("shelled"),
        probe: script,
        transitions: gmr_core::Transitions(vec![gmr_core::Rule {
            when: gmr_core::Expr::text("obs.whatever != state.whatever"),
            to: gmr_core::Expr::text("{ whatever: obs.whatever }"),
        }]),
        terminal: Default::default(),
        initial: None,
        settings: Default::default(),
        supersedes: None,
    };
    assert!(
        w.runtime.open(asked).await.is_ok(),
        "most probes are somebody else's program and this build cannot read what they \
         print. Refusing there would make the check a ban on shell probes rather than a \
         check on rules"
    );
}

#[tokio::test]
async fn a_declaration_the_program_has_outgrown_is_said_at_open() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("world.json"), r#"{"x":0,"y":1}"#).unwrap();
    let asks = std::collections::BTreeMap::from([(
        gmr_core::ProbeName::new("world"),
        gmr_transport::file::Ask::at("world.json"),
    )]);
    let rt = Runtime::builder()
        .transport(Arc::new(gmr_transport::file::Files::new(dir.path(), asks)))
        .journal(Arc::new(MemoryJournal::default()))
        .bindings(Arc::new(MemoryBindings::default()))
        .sealer(Arc::new(MemoryBindings::default()))
        .links(Arc::new(MemoryBindings::default()))
        .settings(Arc::new(MemoryQueue::default()))
        .sightings(Arc::new(MemoryQueue::default()))
        .build();

    let opened = rt
        .open(gmr_runtime::OpenRequest {
            key: gmr_core::AnchorKey::new("wide"),
            probe: gmr_core::ProbeRef::new(
                gmr_core::Kind::new("file"),
                gmr_core::ProbeName::new("world"),
                serde_json::Value::Null,
            ),
            transitions: gmr_core::Transitions(vec![gmr_core::Rule {
                when: gmr_core::Expr::text("obs.value.x != state.x"),
                to: gmr_core::Expr::text("{ x: obs.value.x }"),
            }]),
            terminal: Default::default(),
            initial: None,
            settings: Default::default(),
            supersedes: None,
        })
        .await
        .unwrap();
    assert!(
        opened.warnings.is_empty(),
        "a file probe declares `value` and puts everything under it, so nothing is behind: \
         {:?}",
        opened.warnings
    );
}

#[tokio::test]
async fn a_probe_reporting_more_than_it_declares_says_so_at_open() {
    let registered = std::collections::BTreeMap::from([(
        gmr_core::ProbeName::new("narrow"),
        gmr_transport::inproc::Registered {
            version: gmr_core::ProbeVersion::try_new("c".repeat(64)).unwrap(),
            verifiability: gmr_core::Verifiability::Closed,
            observes: gmr_core::Observes::named(["x"]),
            extract: Arc::new(|_| Ok(serde_json::json!({ "x": 1, "y": 2 }))),
        },
    )]);
    let rt = Runtime::builder()
        .transport(Arc::new(gmr_transport::inproc::InProcess::new(
            ".", registered,
        )))
        .journal(Arc::new(MemoryJournal::default()))
        .bindings(Arc::new(MemoryBindings::default()))
        .sealer(Arc::new(MemoryBindings::default()))
        .links(Arc::new(MemoryBindings::default()))
        .settings(Arc::new(MemoryQueue::default()))
        .sightings(Arc::new(MemoryQueue::default()))
        .build();

    let opened = rt
        .open(gmr_runtime::OpenRequest {
            key: gmr_core::AnchorKey::new("behind"),
            probe: gmr_core::ProbeRef::new(
                gmr_core::Kind::new("builtin"),
                gmr_core::ProbeName::new("narrow"),
                serde_json::Value::Null,
            ),
            transitions: gmr_core::Transitions(vec![gmr_core::Rule {
                when: gmr_core::Expr::text("obs.x != state.x"),
                to: gmr_core::Expr::text("{ x: obs.x }"),
            }]),
            terminal: Default::default(),
            initial: None,
            settings: Default::default(),
            supersedes: None,
        })
        .await
        .unwrap();

    assert!(
        opened.warnings.iter().any(|w| w.contains("obs.y")),
        "the declaration is now what `open` refuses rules against, so a declaration the \
         program has outgrown turns away a rule reading something the probe demonstrably \
         reports. The first real observation is the only moment anyone can notice, and it \
         costs a set comparison on data already in hand: {:?}",
        opened.warnings
    );
    assert!(
        !opened.warnings.iter().any(|w| w.contains("obs.x")),
        "what it did declare is not reported as a surprise"
    );
}

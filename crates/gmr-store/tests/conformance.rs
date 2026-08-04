use gmr_core::{
    Anchor, AnchorKey, Binding, Entry, Expr, FactAddress, Facts, Kind, Observation, Outcome,
    ProbeName, ProbeRef, ProbeVersion, ReasonClass, Ref, Rule, State, Transitions, Version,
    Versions, fold,
};
use gmr_store::{BindingStore, ErrorCode, ErrorKind, Fence, Journal, Sealer};

fn at(n: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_700_000_000 + n, 0).unwrap()
}

fn probe() -> ProbeRef {
    ProbeRef::new(
        Kind::new("shell"),
        ProbeName::new("p"),
        serde_json::json!({}),
    )
}

fn transitions(names: &[&str]) -> Transitions {
    Transitions(
        names
            .iter()
            .map(|n| Rule {
                when: Expr::text(format!("changed(\"{n}\")")),
                to: Expr::text(format!("{{ {n}: obs.{n} }}")),
            })
            .collect(),
    )
}

fn anchor(key: &str, names: &[&str]) -> Anchor {
    Anchor {
        key: AnchorKey::new(key),
        probe: probe(),
        transitions: transitions(names),
        terminal: Default::default(),
        supersedes: None,
    }
}

fn observation(pairs: &[(&str, serde_json::Value)]) -> Observation {
    Observation {
        outcome: Outcome::Found {
            facts: Facts::new(
                pairs
                    .iter()
                    .map(|(n, v)| ((*n).to_owned(), v.clone()))
                    .collect::<serde_json::Map<_, _>>()
                    .into(),
            ),
        },
        fact_address: FactAddress::new("b".repeat(64)),
        versions: Versions {
            declaration: gmr_core::ContentHash::new("d".repeat(64)),
            derivation: gmr_core::Derivation {
                version: ProbeVersion::new("a".repeat(64)),
                verifiability: gmr_core::Verifiability::Closed,
            },
            evaluator: "eval-1".to_owned(),
        },
    }
}

fn state_of(names: &[&str], v: &serde_json::Value) -> State {
    State::new(
        names
            .iter()
            .map(|n| ((*n).to_owned(), v.clone()))
            .collect::<serde_json::Map<_, _>>()
            .into(),
    )
}

fn open_entry(key: &str, names: &[&str], v: serde_json::Value) -> Entry {
    Entry::Open {
        anchor: Box::new(anchor(key, names)),
        observation: observation(&names.iter().map(|n| (*n, v.clone())).collect::<Vec<_>>()),
        state: state_of(names, &v),
        at: at(0),
    }
}

async fn journal_hands_back_what_it_was_given<J: Journal>(j: &J) {
    let key = AnchorKey::new("core::modules");
    let entry = open_entry("core::modules", &["count"], serde_json::json!(5));

    let seq = j.append(&key, &entry, Fence::Unleased).await.unwrap();
    let back = j.entries(&key, 0).await.unwrap();

    assert_eq!(back.len(), 1);
    assert_eq!(back[0].0, seq);
    assert_eq!(
        back[0].1, entry,
        "stored and loaded entries must be identical"
    );
}

async fn journal_is_ordered_and_scoped_per_anchor<J: Journal>(j: &J) {
    let a = AnchorKey::new("a");
    let b = AnchorKey::new("b");

    j.append(
        &a,
        &open_entry("a", &["x"], serde_json::json!(1)),
        Fence::Unleased,
    )
    .await
    .unwrap();
    j.append(
        &b,
        &open_entry("b", &["x"], serde_json::json!(2)),
        Fence::Unleased,
    )
    .await
    .unwrap();
    j.append(
        &a,
        &Entry::Attempt {
            reason: ReasonClass::Unreachable,
            code: None,
            message: "boom".into(),
            at: at(10),
        },
        Fence::Unleased,
    )
    .await
    .unwrap();

    let entries = j.entries(&a, 0).await.unwrap();
    assert_eq!(entries.len(), 2, "b entries must not appear in a's journal");
    assert!(entries[0].0 < entries[1].0, "seq must be monotonic");

    let mut anchors = j.anchors().await.unwrap();
    anchors.sort();
    assert_eq!(anchors, vec![a, b]);
}

fn author_entry() -> Entry {
    Entry::Close {
        context: gmr_core::ContentHash::new("e".repeat(64)),
        rationale: gmr_core::ContentHash::new("f".repeat(64)),
        at: at(20),
    }
}

async fn journal_refuses_a_stale_fencing_token<J: Journal>(j: &J) {
    let key = AnchorKey::new("fenced");
    let entry = open_entry("fenced", &["x"], serde_json::json!(1));

    j.append(&key, &entry, Fence::Unleased).await.unwrap();
    j.append(&key, &entry, Fence::Unleased).await.unwrap();

    j.append(&key, &entry, Fence::Held(7)).await.unwrap();

    let err = j.append(&key, &entry, Fence::Held(3)).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::Constraint);
    assert_eq!(err.code, ErrorCode::StaleFence);
    assert!(
        !err.kind.is_retryable(),
        "retrying a stale token cannot help"
    );

    j.append(&key, &entry, Fence::Held(8)).await.unwrap();

    // This anchor is lease-managed. Slipping in another observation from the
    // side is exactly the second writer the lease exists to prevent. Author
    // revisions are exempt because they are not observations.
    let err = j.append(&key, &entry, Fence::Unleased).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::Constraint);
    assert_eq!(err.code, ErrorCode::LeaseManagedObservation);

    j.append(&key, &author_entry(), Fence::Unleased)
        .await
        .unwrap();
    assert_eq!(j.entries(&key, 0).await.unwrap().len(), 5);
}

async fn a_stored_log_folds_back_into_state<J: Journal>(j: &J) {
    let key = AnchorKey::new("folded");
    j.append(
        &key,
        &open_entry("folded", &["shape"], serde_json::json!("old")),
        Fence::Unleased,
    )
    .await
    .unwrap();
    j.append(
        &key,
        &Entry::Transition {
            observation: observation(&[("shape", serde_json::json!("new"))]),
            state: state_of(&["shape"], &serde_json::json!("new")),
            at: at(10),
        },
        Fence::Unleased,
    )
    .await
    .unwrap();

    let state = fold(&j.entries(&key, 0).await.unwrap()).unwrap();
    assert_eq!(state.state.as_value()["shape"], serde_json::json!("new"));
    assert_eq!(state.attempts, 0);
}

async fn bindings_record_the_version_they_bound<B: BindingStore>(b: &B) {
    let binding = Binding {
        reference: Ref::new("git", "memories/core-modules.md"),
        anchors: vec![AnchorKey::new("core::modules")],
    };
    let bound_version = Version::new("blob-v1");
    b.bind(&binding, &bound_version, Some(7)).await.unwrap();

    let on = b
        .bindings_on(&AnchorKey::new("core::modules"))
        .await
        .unwrap();
    assert_eq!(
        on,
        vec![gmr_store::BindingRecord {
            binding: binding.clone(),
            bound_version: bound_version.clone(),
            bound_at_seq: Some(7),
        }]
    );
    assert_eq!(
        b.binding_of(&binding.reference)
            .await
            .unwrap()
            .unwrap()
            .bound_version,
        Version::new("blob-v1"),
        "the bound version must be retained from day one"
    );
}

async fn a_binding_naming_several_anchors_has_no_single_bound_at_seq<B: BindingStore>(b: &B) {
    let binding = Binding {
        reference: Ref::new("git", "memories/shared.md"),
        anchors: vec![AnchorKey::new("a"), AnchorKey::new("b")],
    };
    b.bind(&binding, &Version::new("v"), None).await.unwrap();

    assert_eq!(
        b.binding_of(&binding.reference)
            .await
            .unwrap()
            .unwrap()
            .bound_at_seq,
        None,
        "which anchor's head would this be? there is no single answer, so it is not stored"
    );
}

async fn rebinding_appends_and_the_latest_wins<B: BindingStore>(b: &B) {
    let reference = Ref::new("git", "memories/m.md");
    for v in ["v1", "v2"] {
        b.bind(
            &Binding {
                reference: reference.clone(),
                anchors: vec![AnchorKey::new("a")],
            },
            &Version::new(v),
            None,
        )
        .await
        .unwrap();
    }

    assert_eq!(
        b.binding_of(&reference)
            .await
            .unwrap()
            .unwrap()
            .bound_version,
        Version::new("v2")
    );
    assert_eq!(
        b.all().await.unwrap().len(),
        1,
        "one reference counts once in the current view"
    );
    assert_eq!(b.bindings_on(&AnchorKey::new("a")).await.unwrap().len(), 1);
}

async fn rebinding_can_move_a_record_off_an_anchor<B: BindingStore>(b: &B) {
    let reference = Ref::new("git", "memories/moved.md");
    for anchor in ["from", "to"] {
        b.bind(
            &Binding {
                reference: reference.clone(),
                anchors: vec![AnchorKey::new(anchor)],
            },
            &Version::new("v"),
            None,
        )
        .await
        .unwrap();
    }
    assert!(
        b.bindings_on(&AnchorKey::new("from"))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(b.bindings_on(&AnchorKey::new("to")).await.unwrap().len(), 1);
}

async fn sealing_is_content_addressed_and_idempotent<S: Sealer>(b: &S) {
    let bytes = "I accepted the move, but not the signature change".as_bytes();
    let a1 = b.seal(bytes).await.unwrap();
    let a2 = b.seal(bytes).await.unwrap();
    assert_eq!(
        a1, a2,
        "same bytes have the same address; repeated sealing is idempotent"
    );
    assert_eq!(b.sealed(&a1).await.unwrap().as_deref(), Some(bytes));
    assert_ne!(a1, b.seal("another rationale".as_bytes()).await.unwrap());
}

async fn links_are_scoped_to_the_from_reference<L: gmr_store::LinkStore>(l: &L) {
    let a = Ref::new("git", "memories/a.md");
    let b = Ref::new("git", "memories/b.md");
    let c = Ref::new("git", "memories/c.md");

    l.link(&a, &b, gmr_core::LinkKind("elaborates".into()))
        .await
        .unwrap();
    l.link(&a, &c, gmr_core::LinkKind("contradicts".into()))
        .await
        .unwrap();

    let from_a = l.links_of(&a).await.unwrap();
    assert_eq!(from_a.len(), 2);
    assert!(from_a.iter().any(|link| link.to == b));
    assert!(from_a.iter().any(|link| link.to == c));

    assert!(
        l.links_of(&b).await.unwrap().is_empty(),
        "links are directed: b was linked to, not from"
    );
}

macro_rules! journal_conformance {
    ($($name:ident),* $(,)?) => {
        mod memory_journal {
            $(#[tokio::test] async fn $name() {
                super::$name(&gmr_store::testkit::MemoryJournal::default()).await;
            })*
        }
        mod sqlite_journal {
            $(#[tokio::test] async fn $name() {
                let store = gmr_store::sqlite::open_in_memory().await.unwrap();
                super::$name(&store.journal()).await;
            })*
        }
    };
}

macro_rules! bindings_conformance {
    ($($name:ident),* $(,)?) => {
        mod memory_bindings {
            $(#[tokio::test] async fn $name() {
                super::$name(&gmr_store::testkit::MemoryBindings::default()).await;
            })*
        }
        mod sqlite_bindings {
            $(#[tokio::test] async fn $name() {
                let store = gmr_store::sqlite::open_in_memory().await.unwrap();
                super::$name(&store.bindings()).await;
            })*
        }
    };
}

macro_rules! links_conformance {
    ($($name:ident),* $(,)?) => {
        mod memory_links {
            $(#[tokio::test] async fn $name() {
                super::$name(&gmr_store::testkit::MemoryBindings::default()).await;
            })*
        }
        mod sqlite_links {
            $(#[tokio::test] async fn $name() {
                let store = gmr_store::sqlite::open_in_memory().await.unwrap();
                super::$name(&store.links()).await;
            })*
        }
    };
}

journal_conformance!(
    journal_hands_back_what_it_was_given,
    journal_is_ordered_and_scoped_per_anchor,
    journal_refuses_a_stale_fencing_token,
    a_stored_log_folds_back_into_state,
);

bindings_conformance!(
    bindings_record_the_version_they_bound,
    a_binding_naming_several_anchors_has_no_single_bound_at_seq,
    rebinding_appends_and_the_latest_wins,
    rebinding_can_move_a_record_off_an_anchor,
    sealing_is_content_addressed_and_idempotent,
);

links_conformance!(links_are_scoped_to_the_from_reference);

async fn queue_contract<Q: gmr_store::Queue>(q: &Q) {
    use chrono::{Duration, TimeZone, Utc};
    use gmr_store::{Disposition, Ticket};

    let t0 = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    let a = AnchorKey::new("a");
    let b = AnchorKey::new("b");
    q.enqueue(&a, t0).await.unwrap();
    q.enqueue(&b, t0 + Duration::seconds(100)).await.unwrap();

    let due = q.due(t0, Duration::seconds(60), 10).await.unwrap();
    assert_eq!(due.len(), 1, "b is not due yet");
    assert_eq!(due[0].anchor, a);
    assert_eq!(
        due[0].fence.epoch(),
        Some(1),
        "each lease increments the epoch, starting at 1"
    );

    let again = q
        .due(t0 + Duration::seconds(10), Duration::seconds(60), 10)
        .await
        .unwrap();
    assert!(
        again.is_empty(),
        "an unexpired lease must not issue the same anchor twice"
    );

    let expired = q
        .due(t0 + Duration::seconds(61), Duration::seconds(60), 10)
        .await
        .unwrap();
    assert_eq!(expired.len(), 1, "an expired lease can be leased again");
    assert_eq!(
        expired[0].fence.epoch(),
        Some(2),
        "epoch keeps increasing so old holders are fenced out of the journal"
    );

    q.settle(
        &expired[0],
        Disposition::Backoff { after_secs: 30 },
        t0 + Duration::seconds(62),
    )
    .await
    .unwrap();
    assert!(
        q.due(t0 + Duration::seconds(80), Duration::seconds(60), 10)
            .await
            .unwrap()
            .is_empty(),
        "backing off"
    );

    let back = q
        .due(t0 + Duration::seconds(100), Duration::seconds(60), 10)
        .await
        .unwrap();
    let names: Vec<&str> = back.iter().map(|t| t.anchor.as_str()).collect();
    assert_eq!(names, vec!["a", "b"]);

    for t in &back {
        q.settle(t, Disposition::Retire, t0 + Duration::seconds(101))
            .await
            .unwrap();
    }
    assert!(
        q.due(t0 + Duration::seconds(999_999), Duration::seconds(60), 10)
            .await
            .unwrap()
            .is_empty(),
        "retired anchors should not be dequeued"
    );

    // If a retired anchor returns, epoch must not move backward. The journal
    // only accepts monotonic tokens and remembers the highest one seen for this
    // anchor; if the queue starts over, every new token will be rejected.
    q.enqueue(&a, t0 + Duration::seconds(1_000)).await.unwrap();
    let reborn = q
        .due(t0 + Duration::seconds(1_000), Duration::seconds(60), 10)
        .await
        .unwrap();
    assert_eq!(reborn.len(), 1);
    assert!(
        reborn[0].fence.epoch().unwrap() > 2,
        "deleting the counter on retire resets the token: got {}, while the journal has already seen 2",
        reborn[0].fence.epoch().unwrap()
    );

    let ghost = Ticket {
        anchor: AnchorKey::new("ghost"),
        fence: gmr_store::Fence::Held(1),
        lease_until: t0,
    };
    q.settle(&ghost, Disposition::Reschedule { after_secs: 1 }, t0)
        .await
        .unwrap();
}

#[tokio::test]
async fn queue_contract_on_testkit() {
    queue_contract(&gmr_store::testkit::MemoryQueue::default()).await;
}

#[tokio::test]
async fn queue_contract_on_sqlite() {
    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    queue_contract(&store.queue()).await;
}

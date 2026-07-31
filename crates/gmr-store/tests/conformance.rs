use gmr_core::{
    Anchor, AnchorKey, Binding, Entry, Expr, FactAddress, Facts, Kind, Observation, Outcome, Probe,
    ProbeVersion, ReasonClass, Ref, Retain, Rule, State, Transitions, Version, Versions, fold,
};
use gmr_store::{BindingStore, ErrorKind, Fence, Journal};

fn at(n: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_700_000_000 + n, 0).unwrap()
}

fn probe(run: &str) -> Probe {
    Probe::new(Kind::new("shell"), serde_json::json!({ "run": run }))
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
        probe: probe("echo '{}'"),
        transitions: transitions(names),
        terminal: Default::default(),
        retain: Retain::Tick,
        cadence_secs: None,
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
        fact_address: Some(FactAddress::new("b".repeat(64))),
        versions: Versions {
            probe: ProbeVersion::new("a".repeat(64)),
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

    let seq = j.append(&key, &entry, Fence::NONE).await.unwrap();
    let back = j.entries(&key, 0).await.unwrap();

    assert_eq!(back.len(), 1);
    assert_eq!(back[0].0, seq);
    assert_eq!(back[0].1, entry, "存进去的和取出来的必须是同一条");
}

async fn journal_is_ordered_and_scoped_per_anchor<J: Journal>(j: &J) {
    let a = AnchorKey::new("a");
    let b = AnchorKey::new("b");

    j.append(
        &a,
        &open_entry("a", &["x"], serde_json::json!(1)),
        Fence::NONE,
    )
    .await
    .unwrap();
    j.append(
        &b,
        &open_entry("b", &["x"], serde_json::json!(2)),
        Fence::NONE,
    )
    .await
    .unwrap();
    j.append(
        &a,
        &Entry::Attempt {
            reason: ReasonClass::Unreachable,
            message: "boom".into(),
            at: at(10),
        },
        Fence::NONE,
    )
    .await
    .unwrap();

    let entries = j.entries(&a, 0).await.unwrap();
    assert_eq!(entries.len(), 2, "b 的条目不该出现在 a 的日志里");
    assert!(entries[0].0 < entries[1].0, "seq 必须单调");

    let mut anchors = j.anchors().await.unwrap();
    anchors.sort();
    assert_eq!(anchors, vec![a, b]);
}

async fn journal_refuses_a_stale_fencing_token<J: Journal>(j: &J) {
    let key = AnchorKey::new("fenced");
    let entry = open_entry("fenced", &["x"], serde_json::json!(1));

    j.append(&key, &entry, Fence::NONE).await.unwrap();
    j.append(&key, &entry, Fence::NONE).await.unwrap();

    j.append(&key, &entry, Fence(7)).await.unwrap();

    let err = j.append(&key, &entry, Fence(3)).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::Constraint);
    assert!(!err.kind.is_retryable(), "过期令牌重试也没用");

    j.append(&key, &entry, Fence(8)).await.unwrap();

    j.append(&key, &entry, Fence::NONE).await.unwrap();
    assert_eq!(j.entries(&key, 0).await.unwrap().len(), 5);
}

async fn a_stored_log_folds_back_into_state<J: Journal>(j: &J) {
    let key = AnchorKey::new("folded");
    j.append(
        &key,
        &open_entry("folded", &["shape"], serde_json::json!("old")),
        Fence::NONE,
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
        Fence::NONE,
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
        bound_version: Version::new("blob-v1"),
        links: vec![],
    };
    b.bind(&binding).await.unwrap();

    let on = b
        .bindings_on(&AnchorKey::new("core::modules"))
        .await
        .unwrap();
    assert_eq!(on, vec![binding.clone()]);
    assert_eq!(
        b.binding_of(&binding.reference)
            .await
            .unwrap()
            .unwrap()
            .bound_version,
        Version::new("blob-v1"),
        "绑定时那一版必须从第一天就记着"
    );
}

async fn rebinding_appends_and_the_latest_wins<B: BindingStore>(b: &B) {
    let reference = Ref::new("git", "memories/m.md");
    for v in ["v1", "v2"] {
        b.bind(&Binding {
            reference: reference.clone(),
            anchors: vec![AnchorKey::new("a")],
            bound_version: Version::new(v),
            links: vec![],
        })
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
    assert_eq!(b.all().await.unwrap().len(), 1, "一条引用只算一次当前值");
    assert_eq!(b.bindings_on(&AnchorKey::new("a")).await.unwrap().len(), 1);
}

async fn rebinding_can_move_a_record_off_an_anchor<B: BindingStore>(b: &B) {
    let reference = Ref::new("git", "memories/moved.md");
    for anchor in ["from", "to"] {
        b.bind(&Binding {
            reference: reference.clone(),
            anchors: vec![AnchorKey::new(anchor)],
            bound_version: Version::new("v"),
            links: vec![],
        })
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

async fn sealing_is_content_addressed_and_idempotent<B: BindingStore>(b: &B) {
    let bytes = "我接受它搬家了，但我没接受签名变化".as_bytes();
    let a1 = b.seal(bytes).await.unwrap();
    let a2 = b.seal(bytes).await.unwrap();
    assert_eq!(a1, a2, "同字节同地址，重复密封是幂等的");
    assert_eq!(b.sealed(&a1).await.unwrap().as_deref(), Some(bytes));
    assert_ne!(a1, b.seal("别的理由".as_bytes()).await.unwrap());
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

journal_conformance!(
    journal_hands_back_what_it_was_given,
    journal_is_ordered_and_scoped_per_anchor,
    journal_refuses_a_stale_fencing_token,
    a_stored_log_folds_back_into_state,
);

bindings_conformance!(
    bindings_record_the_version_they_bound,
    rebinding_appends_and_the_latest_wins,
    rebinding_can_move_a_record_off_an_anchor,
    sealing_is_content_addressed_and_idempotent,
);

async fn queue_contract<Q: gmr_store::Queue>(q: &Q) {
    use chrono::{Duration, TimeZone, Utc};
    use gmr_store::{Disposition, Ticket};

    let t0 = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    let a = AnchorKey::new("a");
    let b = AnchorKey::new("b");
    q.enqueue(&a, t0).await.unwrap();
    q.enqueue(&b, t0 + Duration::seconds(100)).await.unwrap();

    let due = q.due(t0, Duration::seconds(60), 10).await.unwrap();
    assert_eq!(due.len(), 1, "b 还没到点");
    assert_eq!(due[0].anchor, a);
    assert_eq!(due[0].fence.0, 1, "每发一次租约 epoch 递增，从 1 起");

    let again = q
        .due(t0 + Duration::seconds(10), Duration::seconds(60), 10)
        .await
        .unwrap();
    assert!(again.is_empty(), "租约未到期，同一个锚不双发");

    let expired = q
        .due(t0 + Duration::seconds(61), Duration::seconds(60), 10)
        .await
        .unwrap();
    assert_eq!(expired.len(), 1, "租约到期 → 可再租");
    assert_eq!(
        expired[0].fence.0, 2,
        "epoch 继续涨 —— 旧持有者被 fencing 挡在日志外"
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
        "退避中"
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
        "退场后不再出队"
    );

    let ghost = Ticket {
        anchor: AnchorKey::new("ghost"),
        fence: gmr_store::Fence(1),
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

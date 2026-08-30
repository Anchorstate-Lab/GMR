use gmr_core::{
    Anchor, AnchorKey, Binding, Change, Entry, Expr, FactAddress, Facts, Kind, LinkKind,
    Observation, Outcome, ProbeName, ProbeRef, ProbeVersion, Ref, Rule, State, Transitions,
    Version, Versions, fold,
};
use gmr_store::{Asserted, BindingStore, ErrorKind, Expected, Fence, Journal, LinkStore, Sealer};

fn versions() -> Versions {
    Versions {
        declaration: gmr_core::ContentHash::try_new("d".repeat(64)).unwrap(),
        derivation: gmr_core::Derivation {
            version: ProbeVersion::try_new("a".repeat(64)).unwrap(),
            verifiability: gmr_core::Verifiability::Closed,
        },
        evaluator: "eval-1".to_owned(),
    }
}

fn anchor(key: &str) -> Anchor {
    Anchor {
        key: AnchorKey::new(key),
        probe: ProbeRef::new(
            Kind::new("shell"),
            ProbeName::new("p"),
            serde_json::json!({}),
        ),
        transitions: Transitions(vec![Rule {
            when: Expr::text("changed(\"shape\")"),
            to: Expr::text("{ shape: obs.shape }"),
        }]),
        terminal: Default::default(),
        supersedes: None,
    }
}

fn obs(shape: &str) -> Observation {
    Observation {
        outcome: Outcome::Found {
            facts: Facts::new(serde_json::json!({ "shape": shape })),
        },
        fact_address: FactAddress::try_new("b".repeat(64)).unwrap(),
        versions: versions(),
    }
}

async fn populated() -> gmr_store::sqlite::SqliteStore {
    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    let key = AnchorKey::new("a::one");

    let open_seq = store
        .journal()
        .append(
            &key,
            &Entry::Open {
                anchor: Box::new(anchor("a::one")),
                observation: obs("first"),
                state: State::new(serde_json::json!({ "shape": "first" })),
                at: chrono::Utc::now(),
            },
            Fence::Unleased,
            Expected::Any,
        )
        .await
        .unwrap();

    store
        .journal()
        .append(
            &key,
            &Entry::Still {
                ref_entry: open_seq,
                at: chrono::Utc::now(),
                versions: versions(),
            },
            Fence::Unleased,
            Expected::Any,
        )
        .await
        .unwrap();

    let context = store.bindings().seal(b"context bytes").await.unwrap();
    let rationale = store.bindings().seal(b"why we changed it").await.unwrap();
    store
        .journal()
        .append(
            &key,
            &Entry::Revise {
                change: Change::Restate {
                    state: State::new(serde_json::json!({ "shape": "revised" })),
                },
                context,
                rationale,
                at: chrono::Utc::now(),
            },
            Fence::Unleased,
            Expected::Any,
        )
        .await
        .unwrap();

    store
        .bindings()
        .bind(&Asserted {
            binding: Binding::on(Ref::new("git", "memories/one.md"), vec![key.clone()]),
            bound_version: Some(Version::new("v1")),
            bound_at_seq: Some(open_seq),
            saw: None,
            source: gmr_core::Source::Adjudicated,
            at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    store
        .links()
        .link(
            &Ref::new("git", "memories/one.md"),
            &Ref::new("git", "memories/two.md"),
            LinkKind("contradicts".into()),
        )
        .await
        .unwrap();

    store
}

fn without_manifest_line(bytes: &[u8]) -> Vec<&str> {
    std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .skip(1)
        .collect()
}

#[tokio::test]
async fn round_trip_preserves_the_journal_bindings_links_and_sealed_rows() {
    let original = populated().await;

    let mut first = Vec::new();
    original.export_jsonl(&mut first).await.unwrap();

    let restored = gmr_store::sqlite::open_in_memory().await.unwrap();
    let summary = restored.import_jsonl(first.as_slice()).await.unwrap();
    assert_eq!(summary.journal, 3);
    assert_eq!(summary.bindings, 1);
    assert_eq!(summary.binding_anchors, 1);
    assert_eq!(summary.links, 1);
    assert_eq!(summary.sealed, 2);

    let mut second = Vec::new();
    restored.export_jsonl(&mut second).await.unwrap();
    assert_eq!(
        without_manifest_line(&first),
        without_manifest_line(&second),
        "re-exporting the restored store must produce the same rows, seq for seq"
    );

    let key = AnchorKey::new("a::one");
    let original_state = fold(&original.journal().entries(&key, 0).await.unwrap()).unwrap();
    let restored_state = fold(&restored.journal().entries(&key, 0).await.unwrap()).unwrap();
    assert_eq!(original_state.state, restored_state.state);
    assert_eq!(original_state.revisions, restored_state.revisions);
}

#[tokio::test]
async fn import_refuses_a_store_that_already_has_history() {
    let store = populated().await;
    let mut buf = Vec::new();
    store.export_jsonl(&mut buf).await.unwrap();

    let err = store.import_jsonl(buf.as_slice()).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::Constraint);
}

#[tokio::test]
async fn import_refuses_an_export_from_a_different_format_version() {
    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    let bad = "{\"table\":\"manifest\",\"schema\":\"gmr.store-export.v99\",\"exported_at\":\"2024-01-01T00:00:00Z\"}\n";

    let err = store.import_jsonl(bad.as_bytes()).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::Constraint);
}

#[tokio::test]
async fn import_refuses_a_file_with_no_manifest_row() {
    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    let err = store.import_jsonl("".as_bytes()).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::Corrupt);
}

#[tokio::test]
async fn export_does_not_require_the_body_to_match_this_builds_entry_enum() {
    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    sqlx::query("INSERT INTO journal (anchor, fence, body) VALUES ('a', 0, ?1)")
        .bind(r#"{"entry":"some_future_variant","new_field":123}"#)
        .execute(store.pool())
        .await
        .unwrap();

    let mut buf = Vec::new();
    store.export_jsonl(&mut buf).await.unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert!(text.contains("some_future_variant"));
}

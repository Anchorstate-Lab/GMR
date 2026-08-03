use gmr_core::{
    Anchor, AnchorKey, Entry, Expr, FactAddress, Facts, Kind, Observation, Outcome, ProbeRef,
    ProbeVersion, Retain, Rule, State, Transitions, Versions, fold,
};
use gmr_store::{ErrorCode, ErrorKind, Fence, Journal};

fn entry(value: &str) -> Entry {
    Entry::Open {
        anchor: Box::new(Anchor {
            key: AnchorKey::new("core::pure"),
            probe: ProbeRef::new(
                Kind::new("shell"),
                ProbeVersion::new("1".repeat(64)),
                serde_json::json!({}),
            ),
            transitions: Transitions(vec![Rule {
                when: Expr::text("changed(\"shape\")"),
                to: Expr::text("{ shape: obs.shape }"),
            }]),
            terminal: Default::default(),
            retain: Retain::Tick,
            cadence_secs: None,
            supersedes: None,
        }),
        observation: Observation {
            outcome: Outcome::Found {
                facts: Facts::new(serde_json::json!({ "shape": value })),
            },
            fact_address: FactAddress::new("b".repeat(64)),
            versions: Versions {
                declaration: gmr_core::ContentHash::new("d".repeat(64)),
                derivation: gmr_core::Derivation {
                    version: ProbeVersion::new("a".repeat(64)),
                    verifiability: gmr_core::Verifiability::ContentAddressed,
                },
                evaluator: "eval-1".to_owned(),
            },
        },
        state: State::new(serde_json::json!({ "shape": value })),
        at: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn the_log_refuses_rewriting_itself() {
    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    store
        .journal()
        .append(
            &AnchorKey::new("core::pure"),
            &entry("old"),
            Fence::Unleased,
        )
        .await
        .unwrap();

    for sql in [
        "UPDATE journal SET body = '{}' WHERE seq = 1",
        "DELETE FROM journal WHERE seq = 1",
    ] {
        let err = sqlx::query(sql)
            .execute(store.pool())
            .await
            .err()
            .unwrap_or_else(|| panic!("{sql} should have been blocked by a trigger"));
        assert!(
            err.to_string().contains(ErrorCode::AppendOnly.as_str()),
            "{err}"
        );
    }
}

#[tokio::test]
async fn sealed_records_refuse_to_be_rewritten() {
    use gmr_store::Sealer;
    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    let address = store.bindings().seal(b"why").await.unwrap();

    let err = sqlx::query("UPDATE sealed SET body = 'x' WHERE address = ?1")
        .bind(address.as_str())
        .execute(store.pool())
        .await
        .expect_err("sealed records should be protected by a trigger");
    assert!(
        err.to_string()
            .contains(ErrorCode::SealedImmutable.as_str())
    );
}

#[tokio::test]
async fn a_state_outlives_the_process_that_captured_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.db");
    let key = AnchorKey::new("core::pure");

    {
        let store = gmr_store::sqlite::open(&path).await.unwrap();
        store
            .journal()
            .append(&key, &entry("captured-first"), Fence::Unleased)
            .await
            .unwrap();
        store.close().await;
    }

    let store = gmr_store::sqlite::open(&path).await.unwrap();
    let state = fold(&store.journal().entries(&key, 0).await.unwrap()).unwrap();

    assert_eq!(
        state.state.as_value()["shape"],
        serde_json::json!("captured-first"),
        "the second process must stand on the state frozen by the first process"
    );
}

#[tokio::test]
async fn a_database_from_another_generation_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.db");

    let store = gmr_store::sqlite::open(&path).await.unwrap();
    assert_eq!(
        store.schema_version().await.unwrap(),
        gmr_store::sqlite::schema::SCHEMA_VERSION
    );
    sqlx::query("PRAGMA user_version = 99")
        .execute(store.pool())
        .await
        .unwrap();
    store.close().await;

    let err = gmr_store::sqlite::open(&path).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::Constraint);
    assert_eq!(err.code, ErrorCode::SchemaVersionMismatch);
}

#[tokio::test]
async fn a_fresh_database_is_intact() {
    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    assert_eq!(store.integrity().await.unwrap(), "ok");
}

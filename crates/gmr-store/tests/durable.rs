use gmr_core::{
    Anchor, AnchorKey, Entry, Expr, FactAddress, Facts, Kind, Observation, Outcome, ProbeName,
    ProbeRef, ProbeVersion, Rule, State, Transitions, Versions, fold,
};
use gmr_store::{ErrorCode, ErrorKind, Fence, Journal};

fn entry(value: &str) -> Entry {
    Entry::Open {
        anchor: Box::new(Anchor {
            key: AnchorKey::new("core::pure"),
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
        }),
        observation: Observation {
            outcome: Outcome::Found {
                facts: Facts::new(serde_json::json!({ "shape": value })),
            },
            fact_address: FactAddress::try_new("b".repeat(64)).unwrap(),
            versions: Versions {
                declaration: gmr_core::ContentHash::try_new("d".repeat(64)).unwrap(),
                derivation: gmr_core::Derivation {
                    version: ProbeVersion::try_new("a".repeat(64)).unwrap(),
                    verifiability: gmr_core::Verifiability::Closed,
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
async fn run_settings_are_meant_to_be_overwritten() {
    use gmr_core::{Retain, RunSettings};
    use gmr_store::Settings;

    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    let q = store.queue();
    let key = AnchorKey::new("core::pure");

    assert_eq!(q.get(&key).await.unwrap(), None, "nothing was ever set");

    let full = RunSettings {
        facts: gmr_core::Recorded::Plain,
        budget_ms: None,
        retain: Retain::Full,
        cadence_secs: Some(900),
    };
    q.put(&key, &full).await.unwrap();
    assert_eq!(q.get(&key).await.unwrap(), Some(full));

    let tick = RunSettings {
        facts: gmr_core::Recorded::Plain,
        budget_ms: None,
        retain: Retain::Tick,
        cadence_secs: None,
    };
    q.put(&key, &tick).await.unwrap();
    assert_eq!(
        q.get(&key).await.unwrap(),
        Some(tick),
        "a second put replaces the first; no trigger stands in the way"
    );
}

#[tokio::test]
async fn a_retention_the_store_cannot_read_is_corruption() {
    use gmr_store::Settings;

    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    sqlx::query("INSERT INTO settings (anchor, retain, cadence_secs) VALUES ('a', 'sideways', 60)")
        .execute(store.pool())
        .await
        .unwrap();

    let err = store
        .queue()
        .get(&AnchorKey::new("a"))
        .await
        .expect_err("an unreadable retention must not quietly become the default");
    assert_eq!(err.kind, ErrorKind::Corrupt);
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

#[tokio::test]
async fn the_journal_links_every_entry_onto_the_one_before_it() {
    use gmr_store::{Chained, Fence, Journal};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.db");
    let store = gmr_store::sqlite::open(&path).await.unwrap();
    let journal = store.journal();

    let a = gmr_core::AnchorKey::new("a");
    let b = gmr_core::AnchorKey::new("b");
    for (key, at) in [(&a, 1), (&b, 2), (&a, 3)] {
        journal
            .append(key, &attempt(at), Fence::Unleased)
            .await
            .unwrap();
    }

    assert_eq!(
        journal.chain_break().await.unwrap(),
        None,
        "three entries across two anchors chain in one line, because the seq they are \
         ordered by is one global counter"
    );

    store.close().await;

    let raw = sqlx_connect(&path).await;
    sqlx::query("PRAGMA writable_schema = ON")
        .execute(&raw)
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER journal_no_update")
        .execute(&raw)
        .await
        .unwrap();
    sqlx::query("UPDATE journal SET body = replace(body, 'attempt-2', 'attempt-9') WHERE seq = 2")
        .execute(&raw)
        .await
        .unwrap();
    raw.close().await;

    let store = gmr_store::sqlite::open(&path).await.unwrap();
    assert_eq!(
        store.journal().chain_break().await.unwrap(),
        Some(2),
        "rewriting a body in place is what the append-only trigger stops and what the chain \
         is for when something gets past it: the row still parses, and its own hash no \
         longer covers it"
    );
}

async fn sqlx_connect(path: &std::path::Path) -> sqlx::SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(path))
        .await
        .unwrap()
}

fn attempt(n: u32) -> gmr_core::Entry {
    gmr_core::Entry::Attempt {
        reason: gmr_core::ReasonClass::Unreachable,
        code: None,
        message: format!("attempt-{n}"),
        at: chrono::DateTime::from_timestamp(n as i64, 0).unwrap(),
    }
}

#[tokio::test]
async fn two_writers_on_one_journal_lose_nothing_and_leave_the_chain_whole() {
    use gmr_store::{Chained, Fence, Journal};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.db");
    gmr_store::sqlite::open(&path).await.unwrap().close().await;

    const EACH: u32 = 40;
    let writing: Vec<_> = (0..2u32)
        .map(|who| {
            let path = path.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async move {
                    let store = gmr_store::sqlite::open(&path).await.unwrap();
                    let journal = store.journal();
                    let key = gmr_core::AnchorKey::new(format!("anchor-{who}"));
                    for n in 0..EACH {
                        journal
                            .append(&key, &attempt(who * 1000 + n), Fence::Unleased)
                            .await
                            .expect("a second writer is contention, not a failure");
                    }
                    store.close().await;
                });
            })
        })
        .collect();
    for w in writing {
        w.join().unwrap();
    }

    let store = gmr_store::sqlite::open(&path).await.unwrap();
    let journal = store.journal();
    for who in 0..2u32 {
        assert_eq!(
            journal
                .entries(&gmr_core::AnchorKey::new(format!("anchor-{who}")), 0)
                .await
                .unwrap()
                .len() as u32,
            EACH,
            "nothing either writer appended may go missing"
        );
    }
    assert_eq!(
        journal.chain_break().await.unwrap(),
        None,
        "and the two interleaved streams are still one unbroken line. `pool.begin()` is \
         BEGIN DEFERRED, which takes the read lock first and upgrades on the write -- under \
         WAL the second writer meets BUSY_SNAPSHOT, which busy_timeout does not retry, and \
         a chain read before that write would link onto a tail somebody else had already \
         moved. BEGIN IMMEDIATE takes the write lock up front, so contention becomes a wait"
    );
}

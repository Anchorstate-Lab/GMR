use std::sync::{Arc, Barrier};

use gmr_survey::index::{Generation, Index, Indexed, Row};
use gmr_survey::sqlite::{SCHEMA_VERSION, open};

fn at(n: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_700_000_000 + n, 0).unwrap()
}

fn file(rel: &str) -> Indexed {
    Indexed {
        rel: rel.to_owned(),
        hash: "h".to_owned(),
        sort: rel.replace('/', "\u{0}"),
        rows: vec![Row {
            ord: 0,
            id: rel.to_owned(),
            coord: [("file".to_owned(), rel.to_owned())].into(),
            facts: serde_json::json!({}),
        }],
    }
}

async fn stamp_of(index: &gmr_survey::sqlite::SqliteIndex) -> i64 {
    sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(index.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn an_index_survives_the_process_that_wrote_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.db");
    let ast = Generation::of("ast-map", "v1");

    let first = open(&path).await.unwrap();
    first.write(&ast, &[file("a.rs")]).await.unwrap();
    first.seal(&ast, at(0)).await.unwrap();
    first.close().await;

    let second = open(&path).await.unwrap();
    let built = second.built(&ast).await.unwrap().expect("it was written");
    assert_eq!((built.files, built.rows), (1, 1));
    assert_eq!(built.sealed_at, Some(at(0)));
    assert_eq!(second.rows(&ast, "").await.unwrap().len(), 1);
    second.close().await;
}

#[tokio::test]
async fn an_index_this_build_cannot_read_is_rebuilt_rather_than_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.db");

    let stale = open(&path).await.unwrap();
    sqlx::raw_sql("CREATE TABLE relic (x INTEGER); PRAGMA user_version = 99")
        .execute(stale.pool())
        .await
        .unwrap();
    stale.close().await;

    let fresh = open(&path).await.unwrap();
    assert_eq!(
        stamp_of(&fresh).await,
        SCHEMA_VERSION,
        "an index is derived data. A journal from another generation has to be refused \
         because what it holds cannot be recomputed; an index can always be rebuilt from \
         the repository, so refusing to open it would cost a person their afternoon to \
         save a scan"
    );
    let survivors: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'relic'",
    )
    .fetch_all(fresh.pool())
    .await
    .unwrap();
    assert!(
        survivors.is_empty(),
        "a shape this build does not know is dropped, not left to be half-read"
    );

    let ast = Generation::of("ast-map", "v1");
    fresh.write(&ast, &[file("a.rs")]).await.unwrap();
    assert_eq!(fresh.built(&ast).await.unwrap().unwrap().files, 1);
    fresh.close().await;
}

fn opened(path: std::path::PathBuf, gate: Arc<Barrier>) -> Result<i64, String> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    gate.wait();
    rt.block_on(async move {
        let index = open(&path).await.map_err(|e| e.to_string())?;
        let stamp = stamp_of(&index).await;
        index.close().await;
        Ok(stamp)
    })
}

#[test]
fn two_processes_opening_one_stale_index_both_land() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.db");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let stale = open(&path).await.unwrap();
        sqlx::raw_sql("PRAGMA user_version = 99")
            .execute(stale.pool())
            .await
            .unwrap();
        stale.close().await;
    });

    let gate = Arc::new(Barrier::new(2));
    let running: Vec<_> = (0..2)
        .map(|_| {
            let (path, gate) = (path.clone(), Arc::clone(&gate));
            std::thread::spawn(move || opened(path, gate))
        })
        .collect();
    let landed: Vec<_> = running.into_iter().map(|h| h.join().unwrap()).collect();

    for outcome in landed {
        assert_eq!(
            outcome,
            Ok(SCHEMA_VERSION),
            "both openers have to land: the second one arrives after the first has \
             already rebuilt, reads the stamp inside the write lock, and finds there is \
             nothing left to do. Deciding outside the lock is what made the journal's \
             first upgrade fail with a bare SQLite error"
        );
    }
}

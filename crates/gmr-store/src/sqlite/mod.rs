pub mod bindings;
pub mod journal;
pub mod links;
pub mod portable;
pub mod queue;
pub mod schema;
pub mod settings;
pub mod sightings;
pub mod usage;

use std::path::Path;

use crate::{ErrorCode, ErrorKind, StoreError};
use gmr_core::{Claim, Ref, canonicalize};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

pub use bindings::SqliteBindings;
pub use journal::SqliteJournal;
pub use portable::{EXPORT_SCHEMA, PortableSummary};
pub use queue::SqliteQueue;

pub(crate) fn ref_key(r: &Ref) -> String {
    keyed(&serde_json::to_value(r).expect("a Ref always serialises"))
}

pub(crate) fn claim_key(c: &Claim) -> String {
    keyed(&c.identity())
}

fn keyed(value: &serde_json::Value) -> String {
    let bytes = canonicalize(value).expect("a reference never exceeds canonicalization limits");
    String::from_utf8(bytes).expect("canonical JSON is always UTF-8")
}

pub(crate) fn db_err(e: sqlx::Error) -> StoreError {
    let (kind, code) = match &e {
        sqlx::Error::Database(db) => {
            let message = db.message();
            let code = if message.contains("append_only") {
                ErrorCode::AppendOnly
            } else if message.contains("sealed_immutable") {
                ErrorCode::SealedImmutable
            } else {
                ErrorCode::Constraint
            };
            (ErrorKind::Constraint, code)
        }
        sqlx::Error::Io(_) => (ErrorKind::Io, ErrorCode::Io),
        sqlx::Error::PoolTimedOut => (ErrorKind::Busy, ErrorCode::Busy),
        _ => (ErrorKind::Other, ErrorCode::Other),
    };
    StoreError::with_code(kind, code, e.to_string())
}

pub(crate) fn decode_err(e: serde_json::Error) -> StoreError {
    StoreError::corrupt(format!("the stored bytes are not what they should be: {e}"))
}

pub async fn open(path: impl AsRef<Path>) -> Result<SqliteStore, StoreError> {
    open_with(path, Pooling::default()).await
}

pub async fn open_with(
    path: impl AsRef<Path>,
    pooling: Pooling,
) -> Result<SqliteStore, StoreError> {
    let options = SqliteConnectOptions::new()
        .filename(path.as_ref())
        .create_if_missing(true);
    connect(options, pooling).await
}

pub async fn open_in_memory() -> Result<SqliteStore, StoreError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(SqliteConnectOptions::new().in_memory(true))
        .await
        .map_err(db_err)?;
    migrate(&pool).await?;
    Ok(SqliteStore { pool })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pooling {
    pub max_connections: u32,
    pub busy_timeout: std::time::Duration,
}

impl Default for Pooling {
    fn default() -> Self {
        Self {
            max_connections: 4,
            busy_timeout: std::time::Duration::from_secs(5),
        }
    }
}

async fn connect(
    options: SqliteConnectOptions,
    pooling: Pooling,
) -> Result<SqliteStore, StoreError> {
    let options = options.busy_timeout(pooling.busy_timeout);
    let pool = SqlitePoolOptions::new()
        .max_connections(pooling.max_connections)
        .connect_with(options)
        .await
        .map_err(db_err)?;
    migrate(&pool).await?;
    Ok(SqliteStore { pool })
}

#[derive(Clone, Copy)]
pub(crate) enum Rung {
    Sql(&'static str),
    Chain,
}

pub(crate) const LADDER: &[(i64, Rung)] = &[
    (6, Rung::Sql(schema::V6_TO_V7)),
    (7, Rung::Sql(schema::V7_TO_V8)),
    (8, Rung::Sql(schema::V8_TO_V9)),
    (9, Rung::Sql(schema::V9_TO_V10)),
    (10, Rung::Chain),
    (11, Rung::Sql(schema::V11_TO_V12)),
    (12, Rung::Sql(schema::V12_TO_V13)),
    (13, Rung::Sql(schema::V13_TO_V14)),
    (14, Rung::Sql(schema::V14_TO_V15)),
];

async fn migrate(pool: &SqlitePool) -> Result<(), StoreError> {
    let stamped: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

    if stamped == schema::SCHEMA_VERSION {
        return Ok(());
    }
    if stamped > schema::SCHEMA_VERSION {
        return Err(from_the_future(stamped));
    }
    climb(pool, schema::SCHEMA_VERSION, LADDER).await
}

fn from_the_future(stamped: i64) -> StoreError {
    StoreError::with_code(
        ErrorKind::Constraint,
        ErrorCode::SchemaVersionMismatch,
        format!(
            "this database is stamped schema v{stamped}, and this build only knows v{}. \
             Refusing to open — misreading a database written by a later generation is \
             far worse than not opening it. Upgrade this binary: \
             `npm i -g @anchorstate-lab/gmr@latest`, or \
             `curl -fsSL https://raw.githubusercontent.com/Anchorstate-Lab/GMR/main/dist/install.sh | sh`",
            schema::SCHEMA_VERSION
        ),
    )
}

enum Climbed {
    Landed,
    Again,
}

async fn climb(pool: &SqlitePool, to: i64, ladder: &[(i64, Rung)]) -> Result<(), StoreError> {
    while let Climbed::Again = rung(pool, to, ladder).await? {}
    Ok(())
}

async fn rung(pool: &SqlitePool, to: i64, ladder: &[(i64, Rung)]) -> Result<Climbed, StoreError> {
    let mut held = pool.acquire().await.map_err(db_err)?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *held)
        .await
        .map_err(db_err)?;

    let climbed = under_the_write_lock(&mut held, to, ladder).await;
    let closed = sqlx::query(match climbed.is_ok() {
        true => "COMMIT",
        false => "ROLLBACK",
    })
    .execute(&mut *held)
    .await;

    let climbed = climbed?;
    closed.map_err(db_err)?;
    Ok(climbed)
}

async fn under_the_write_lock(
    held: &mut sqlx::SqliteConnection,
    to: i64,
    ladder: &[(i64, Rung)],
) -> Result<Climbed, StoreError> {
    let at: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *held)
        .await
        .map_err(db_err)?;

    if at == to {
        return Ok(Climbed::Landed);
    }
    if at > to {
        return Err(from_the_future(at));
    }

    let (next, step) = match at {
        0 => (to, Rung::Sql(schema::SCHEMA)),
        _ => (
            at + 1,
            ladder
                .iter()
                .find(|(rung, _)| *rung == at)
                .map(|(_, step)| *step)
                .ok_or_else(|| {
                    StoreError::with_code(
                        ErrorKind::Constraint,
                        ErrorCode::SchemaVersionMismatch,
                        format!(
                            "this database is stamped schema v{at} and this build is v{to}, \
                             but nothing in this build knows how to carry a v{at} across. \
                             Export it with the version of gmr that wrote it, then import here"
                        ),
                    )
                })?,
        ),
    };

    match step {
        Rung::Sql(sql) => statements(&mut *held, sql).await?,
        Rung::Chain => chain_the_journal(&mut *held).await?,
    }
    sqlx::query(&format!("PRAGMA user_version = {next}"))
        .execute(&mut *held)
        .await
        .map_err(db_err)?;

    Ok(match next == to {
        true => Climbed::Landed,
        false => Climbed::Again,
    })
}

async fn statements(held: &mut sqlx::SqliteConnection, sql: &str) -> Result<(), StoreError> {
    use sqlx::Executor;
    held.execute(sqlx::raw_sql(sql)).await.map_err(db_err)?;
    Ok(())
}

async fn chain_the_journal(held: &mut sqlx::SqliteConnection) -> Result<(), StoreError> {
    use sqlx::Row;

    statements(&mut *held, schema::V10_TO_V11_OPEN).await?;

    let rows = sqlx::query("SELECT seq, anchor, fence, body FROM journal ORDER BY seq")
        .fetch_all(&mut *held)
        .await
        .map_err(db_err)?;

    let mut prev: Option<String> = None;
    for r in rows {
        let anchor = gmr_core::AnchorKey::new(r.get::<String, _>("anchor"));
        let fence = match r.get::<i64, _>("fence") {
            0 => crate::Fence::Unleased,
            n => crate::Fence::Held(n as u64),
        };
        let entry: gmr_core::Entry =
            serde_json::from_str(&r.get::<String, _>("body")).map_err(decode_err)?;
        let hash = crate::journal::link(prev.as_deref(), &anchor, fence, &entry)?;
        sqlx::query("UPDATE journal SET prev = ?1, hash = ?2 WHERE seq = ?3")
            .bind(prev.as_deref())
            .bind(hash.as_str())
            .bind(r.get::<i64, _>("seq"))
            .execute(&mut *held)
            .await
            .map_err(db_err)?;
        prev = Some(hash.into_inner());
    }

    statements(&mut *held, schema::V10_TO_V11_CLOSE).await?;
    Ok(())
}

#[derive(Debug)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub fn journal(&self) -> SqliteJournal {
        SqliteJournal::new(self.pool.clone())
    }

    pub fn bindings(&self) -> SqliteBindings {
        SqliteBindings::new(self.pool.clone())
    }

    pub fn links(&self) -> SqliteBindings {
        SqliteBindings::new(self.pool.clone())
    }

    pub fn sealer(&self) -> SqliteBindings {
        SqliteBindings::new(self.pool.clone())
    }

    pub fn queue(&self) -> SqliteQueue {
        SqliteQueue::new(self.pool.clone())
    }

    pub fn settings(&self) -> SqliteQueue {
        SqliteQueue::new(self.pool.clone())
    }

    pub fn sightings(&self) -> SqliteQueue {
        SqliteQueue::new(self.pool.clone())
    }

    pub fn usage(&self) -> SqliteQueue {
        SqliteQueue::new(self.pool.clone())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }

    pub async fn integrity(&self) -> Result<String, StoreError> {
        sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn raw() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(SqliteConnectOptions::new().in_memory(true))
            .await
            .unwrap()
    }

    async fn stamp(pool: &SqlitePool, at: i64) {
        sqlx::query(&format!("PRAGMA user_version = {at}"))
            .execute(pool)
            .await
            .unwrap();
    }

    async fn stamp_of(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn shape(pool: &SqlitePool) -> Vec<(String, String)> {
        sqlx::query_as(
            "SELECT name, COALESCE(sql, '') FROM sqlite_master \
             WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    const FULL: &str = "CREATE TABLE a (x INTEGER);\n\
                        CREATE TABLE b (y TEXT);\n\
                        CREATE INDEX b_by_y ON b(y);";
    const OLD: &str = "CREATE TABLE a (x INTEGER);";
    const RUNG: &str = "CREATE TABLE b (y TEXT);\nCREATE INDEX b_by_y ON b(y);";

    #[tokio::test]
    async fn a_climbed_database_ends_up_shaped_like_a_freshly_built_one() {
        let fresh = raw().await;
        sqlx::raw_sql(FULL).execute(&fresh).await.unwrap();

        let climbed = raw().await;
        sqlx::raw_sql(OLD).execute(&climbed).await.unwrap();
        stamp(&climbed, 1).await;
        climb(&climbed, 2, &[(1, Rung::Sql(RUNG))]).await.unwrap();

        assert_eq!(
            shape(&fresh).await,
            shape(&climbed).await,
            "building from scratch and climbing the ladder must agree, or the two \
             paths have drifted and only one of them is ever exercised"
        );
        assert_eq!(stamp_of(&climbed).await, 2);
    }

    #[tokio::test]
    async fn that_comparison_can_actually_fail() {
        let fresh = raw().await;
        sqlx::raw_sql(FULL).execute(&fresh).await.unwrap();

        let climbed = raw().await;
        sqlx::raw_sql(OLD).execute(&climbed).await.unwrap();
        stamp(&climbed, 1).await;
        climb(&climbed, 2, &[(1, Rung::Sql("CREATE TABLE b (y TEXT);"))])
            .await
            .unwrap();

        assert_ne!(
            shape(&fresh).await,
            shape(&climbed).await,
            "a rung that forgets the index has to be caught, or the test above proves nothing"
        );
    }

    #[tokio::test]
    async fn every_rung_is_climbed_in_order_and_stamped_as_it_goes() {
        let pool = raw().await;
        let ladder: &[(i64, Rung)] = &[
            (1, Rung::Sql("CREATE TABLE one (x INTEGER);")),
            (2, Rung::Sql("CREATE TABLE two (x INTEGER);")),
            (3, Rung::Sql("CREATE TABLE three (x INTEGER);")),
        ];
        stamp(&pool, 1).await;
        climb(&pool, 4, ladder).await.unwrap();

        assert_eq!(stamp_of(&pool).await, 4);
        let names: Vec<String> = shape(&pool).await.into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["one", "three", "two"]);
    }

    #[tokio::test]
    async fn climbing_starts_where_the_database_is_not_at_the_bottom() {
        let pool = raw().await;
        sqlx::raw_sql("CREATE TABLE one (x INTEGER);")
            .execute(&pool)
            .await
            .unwrap();
        let ladder: &[(i64, Rung)] = &[
            (1, Rung::Sql("CREATE TABLE one (x INTEGER);")),
            (2, Rung::Sql("CREATE TABLE two (x INTEGER);")),
        ];
        stamp(&pool, 2).await;
        climb(&pool, 3, ladder).await.unwrap();

        let names: Vec<String> = shape(&pool).await.into_iter().map(|(n, _)| n).collect();
        assert_eq!(
            names,
            vec!["one", "two"],
            "the v1 rung must not be replayed"
        );
    }

    #[tokio::test]
    async fn a_missing_rung_is_refused_and_says_which_one() {
        let pool = raw().await;
        stamp(&pool, 4).await;
        let e = climb(
            &pool,
            6,
            &[(5, Rung::Sql("CREATE TABLE five (x INTEGER);"))],
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::SchemaVersionMismatch);
        assert!(e.to_string().contains("v4"), "{e}");
        assert_eq!(stamp_of(&pool).await, 4, "a refused climb moves nothing");
    }

    #[tokio::test]
    async fn a_rung_that_fails_leaves_the_stamp_where_it_was() {
        let pool = raw().await;
        sqlx::query("PRAGMA user_version = 1")
            .execute(&pool)
            .await
            .unwrap();
        let ladder: &[(i64, Rung)] = &[
            (1, Rung::Sql("CREATE TABLE ok (x INTEGER);")),
            (2, Rung::Sql("THIS IS NOT SQL;")),
        ];
        assert!(climb(&pool, 3, ladder).await.is_err());
        assert_eq!(
            stamp_of(&pool).await,
            2,
            "the rung that worked is stamped, the one that failed is not — so the \
             next run picks up exactly where this one stopped"
        );
    }

    #[tokio::test]
    async fn a_database_from_a_later_generation_is_still_refused() {
        let store = open_in_memory().await.unwrap();
        sqlx::query(&format!(
            "PRAGMA user_version = {}",
            schema::SCHEMA_VERSION + 1
        ))
        .execute(store.pool())
        .await
        .unwrap();

        let e = migrate(store.pool()).await.unwrap_err();
        assert_eq!(e.code, ErrorCode::SchemaVersionMismatch);
        assert!(
            e.to_string().contains("Upgrade this binary"),
            "the refusal has to carry the way out, not just the verdict: {e}"
        );
    }

    #[tokio::test]
    async fn a_database_already_at_this_version_is_left_alone() {
        let store = open_in_memory().await.unwrap();
        let before = shape(store.pool()).await;
        migrate(store.pool()).await.unwrap();
        assert_eq!(before, shape(store.pool()).await);
        assert_eq!(stamp_of(store.pool()).await, schema::SCHEMA_VERSION);
    }

    const V6_SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS journal (
    seq     INTEGER PRIMARY KEY AUTOINCREMENT,
    anchor  TEXT    NOT NULL,
    fence   INTEGER NOT NULL,
    body    TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS journal_by_anchor ON journal(anchor, seq);

CREATE TABLE IF NOT EXISTS bindings (
    seq            INTEGER PRIMARY KEY AUTOINCREMENT,
    reference      TEXT NOT NULL,
    body           TEXT NOT NULL,
    bound_version  TEXT NOT NULL,
    bound_at_seq   INTEGER
);
CREATE INDEX IF NOT EXISTS bindings_by_reference ON bindings(reference, seq);

CREATE TABLE IF NOT EXISTS binding_anchors (
    seq        INTEGER NOT NULL REFERENCES bindings(seq),
    anchor     TEXT    NOT NULL,
    PRIMARY KEY (seq, anchor)
);
CREATE INDEX IF NOT EXISTS binding_anchors_by_anchor ON binding_anchors(anchor);

CREATE TABLE IF NOT EXISTS links (
    seq      INTEGER PRIMARY KEY AUTOINCREMENT,
    from_ref TEXT NOT NULL,
    to_ref   TEXT NOT NULL,
    kind     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS links_by_from ON links(from_ref);

CREATE TABLE IF NOT EXISTS sealed (
    address  TEXT PRIMARY KEY,
    body     BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    anchor        TEXT    PRIMARY KEY,
    retain        TEXT    NOT NULL,
    cadence_secs  INTEGER
);

CREATE TABLE IF NOT EXISTS queue (
    anchor       TEXT    PRIMARY KEY,
    due          INTEGER NOT NULL,
    lease_until  INTEGER NOT NULL DEFAULT 0,
    epoch        INTEGER NOT NULL DEFAULT 0,
    parked       INTEGER NOT NULL DEFAULT 0
);

CREATE TRIGGER IF NOT EXISTS journal_no_update BEFORE UPDATE ON journal
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS journal_no_delete BEFORE DELETE ON journal
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS bindings_no_update BEFORE UPDATE ON bindings
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS bindings_no_delete BEFORE DELETE ON bindings
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS binding_anchors_no_update BEFORE UPDATE ON binding_anchors
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS binding_anchors_no_delete BEFORE DELETE ON binding_anchors
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS links_no_update BEFORE UPDATE ON links
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS links_no_delete BEFORE DELETE ON links
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS sealed_no_update BEFORE UPDATE ON sealed
    BEGIN SELECT RAISE(ABORT, 'sealed_immutable'); END;
CREATE TRIGGER IF NOT EXISTS sealed_no_delete BEFORE DELETE ON sealed
    BEGIN SELECT RAISE(ABORT, 'sealed_immutable'); END;
"#;

    #[tokio::test]
    async fn a_real_v6_database_is_carried_to_v7_with_what_it_held() {
        let pool = raw().await;
        sqlx::raw_sql(V6_SCHEMA).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO settings VALUES ('a#b', 'full', 900)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA user_version = 6")
            .execute(&pool)
            .await
            .unwrap();

        climb(&pool, 7, LADDER).await.unwrap();

        assert_eq!(stamp_of(&pool).await, 7);
        let (retain, cadence, budget): (String, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT retain, cadence_secs, budget_ms FROM settings WHERE anchor = 'a#b'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!((retain.as_str(), cadence), ("full", Some(900)));
        assert_eq!(
            budget, None,
            "a column that did not exist reads as no opinion"
        );
    }

    #[tokio::test]
    async fn a_whole_v6_database_climbs_into_the_shape_a_fresh_build_makes() {
        let climbed = raw().await;
        sqlx::raw_sql(V6_SCHEMA).execute(&climbed).await.unwrap();
        stamp(&climbed, 6).await;
        climb(&climbed, schema::SCHEMA_VERSION, LADDER)
            .await
            .unwrap();

        let fresh = open_in_memory().await.unwrap();
        assert_eq!(
            blueprint(&climbed).await,
            blueprint(fresh.pool()).await,
            "the ladder and the full schema are two descriptions of one shape, and only \
             this comparison keeps them saying the same thing"
        );
    }

    async fn on_disk(path: &Path) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true)
                    .busy_timeout(std::time::Duration::from_secs(10)),
            )
            .await
            .unwrap()
    }

    const RACED_RUNG: &[(i64, Rung)] = &[(1, Rung::Sql("ALTER TABLE a ADD COLUMN y INTEGER;"))];

    #[test]
    fn two_openers_racing_the_same_upgrade_do_not_both_apply_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.db");

        alone(async {
            let seed = on_disk(&path).await;
            sqlx::raw_sql("CREATE TABLE a (x INTEGER);")
                .execute(&seed)
                .await
                .unwrap();
            sqlx::query("PRAGMA user_version = 1")
                .execute(&seed)
                .await
                .unwrap();
            seed.close().await;
        });

        let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
        let racers: Vec<_> = ["first", "second"]
            .into_iter()
            .map(|who| {
                let (path, gate) = (path.clone(), std::sync::Arc::clone(&gate));
                std::thread::spawn(move || {
                    alone(async {
                        let pool = on_disk(&path).await;
                        gate.wait();
                        let outcome = climb(&pool, 2, RACED_RUNG).await;
                        pool.close().await;
                        (who, outcome)
                    })
                })
            })
            .collect();

        for racer in racers {
            let (who, outcome) = racer.join().unwrap();
            outcome.unwrap_or_else(|e| {
                panic!(
                    "the {who} opener failed: {e}\n\
                     A rung is an ALTER, which is not idempotent. Two processes that both read \
                     the stamp outside any transaction will both try to apply it, and one gets \
                     a bare SQLite error — so the whole SchemaVersionMismatch vocabulary, the \
                     messages written to tell a person what to do, is bypassed on the single \
                     run that ever reaches this path"
                )
            });
        }

        alone(async {
            let held = on_disk(&path).await;
            assert_eq!(stamp_of(&held).await, 2);
            assert_eq!(
                columns(&held, "a").await,
                vec![
                    ("x".to_owned(), "INTEGER".to_owned()),
                    ("y".to_owned(), "INTEGER".to_owned())
                ],
                "the column lands exactly once"
            );
        });
    }

    fn alone<T>(work: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(work)
    }

    async fn columns(pool: &SqlitePool, table: &str) -> Vec<(String, String)> {
        sqlx::query_as(&format!(
            "SELECT name, type FROM pragma_table_info('{table}') ORDER BY name"
        ))
        .fetch_all(pool)
        .await
        .unwrap()
    }

    async fn blueprint(pool: &SqlitePool) -> Vec<String> {
        let objects: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT type, name, COALESCE(sql, '') FROM sqlite_master \
             WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
        )
        .fetch_all(pool)
        .await
        .unwrap();

        let mut drawn = Vec::new();
        for (kind, name, sql) in objects {
            if kind != "table" {
                let squeezed: Vec<&str> = sql.split_whitespace().collect();
                drawn.push(format!("{kind} {name} :: {}", squeezed.join(" ")));
                continue;
            }
            drawn.push(format!("table {name}"));
            let columns: Vec<(String, String, i64, Option<String>, i64)> = sqlx::query_as(&format!(
                "SELECT name, type, \"notnull\", dflt_value, pk FROM pragma_table_info('{name}') \
                 ORDER BY name"
            ))
            .fetch_all(pool)
            .await
            .unwrap();
            for (column, of_type, not_null, default, key) in columns {
                let default = default.unwrap_or_else(|| "-".to_owned());
                drawn.push(format!(
                    "  {column} {of_type} notnull={not_null} default={default} pk={key}"
                ));
            }
        }
        drawn
    }

    #[test]
    fn the_ladder_has_a_rung_for_every_version_it_claims_to_cross() {
        let Some(lowest) = LADDER.iter().map(|(from, _)| *from).min() else {
            return;
        };
        for at in lowest..schema::SCHEMA_VERSION {
            assert!(
                LADDER.iter().any(|(from, _)| *from == at),
                "the ladder starts at v{lowest} but has no rung from v{at}, so a database \
                 stamped v{at} can never be carried across"
            );
        }
    }
}

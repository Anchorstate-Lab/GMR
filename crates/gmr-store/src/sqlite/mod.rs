pub mod bindings;
pub mod journal;
pub mod links;
pub mod portable;
pub mod queue;
pub mod schema;
pub mod settings;

use std::path::Path;

use crate::{ErrorCode, ErrorKind, StoreError};
use gmr_core::{Ref, canonicalize};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

pub use bindings::SqliteBindings;
pub use journal::SqliteJournal;
pub use portable::{EXPORT_SCHEMA, PortableSummary};
pub use queue::SqliteQueue;

pub(crate) fn ref_key(r: &Ref) -> String {
    let bytes = canonicalize(&serde_json::to_value(r).expect("a Ref always serialises"))
        .expect("a Ref never exceeds canonicalization limits");
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
    let options = SqliteConnectOptions::new()
        .filename(path.as_ref())
        .create_if_missing(true);
    connect(options).await
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

async fn connect(options: SqliteConnectOptions) -> Result<SqliteStore, StoreError> {
    let options = options.busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(db_err)?;
    migrate(&pool).await?;
    Ok(SqliteStore { pool })
}

pub(crate) const LADDER: &[(i64, &str)] = &[(6, schema::V6_TO_V7)];

async fn migrate(pool: &SqlitePool) -> Result<(), StoreError> {
    let stamped: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

    if stamped == 0 {
        sqlx::raw_sql(schema::SCHEMA)
            .execute(pool)
            .await
            .map_err(db_err)?;
        sqlx::query(&format!("PRAGMA user_version = {}", schema::SCHEMA_VERSION))
            .execute(pool)
            .await
            .map_err(db_err)?;
        return Ok(());
    }

    if stamped > schema::SCHEMA_VERSION {
        return Err(StoreError::with_code(
            ErrorKind::Constraint,
            ErrorCode::SchemaVersionMismatch,
            format!(
                "this database is stamped schema v{stamped}, and this build only knows v{}. \
                 Refusing to open — misreading a database written by a later generation is \
                 far worse than not opening it. Upgrade gmr",
                schema::SCHEMA_VERSION
            ),
        ));
    }

    climb(pool, stamped, schema::SCHEMA_VERSION, LADDER).await
}

async fn climb(
    pool: &SqlitePool,
    from: i64,
    to: i64,
    ladder: &[(i64, &str)],
) -> Result<(), StoreError> {
    let mut at = from;
    while at < to {
        let step = ladder
            .iter()
            .find(|(rung, _)| *rung == at)
            .map(|(_, sql)| *sql)
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
            })?;

        let mut tx = pool.begin().await.map_err(db_err)?;
        sqlx::raw_sql(step)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query(&format!("PRAGMA user_version = {}", at + 1))
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        at += 1;
    }
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

    pub fn queue(&self) -> SqliteQueue {
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
        climb(&climbed, 1, 2, &[(1, RUNG)]).await.unwrap();

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
        climb(&climbed, 1, 2, &[(1, "CREATE TABLE b (y TEXT);")])
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
        let ladder: &[(i64, &str)] = &[
            (1, "CREATE TABLE one (x INTEGER);"),
            (2, "CREATE TABLE two (x INTEGER);"),
            (3, "CREATE TABLE three (x INTEGER);"),
        ];
        climb(&pool, 1, 4, ladder).await.unwrap();

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
        let ladder: &[(i64, &str)] = &[
            (1, "CREATE TABLE one (x INTEGER);"),
            (2, "CREATE TABLE two (x INTEGER);"),
        ];
        climb(&pool, 2, 3, ladder).await.unwrap();

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
        let e = climb(&pool, 4, 6, &[(5, "CREATE TABLE five (x INTEGER);")])
            .await
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::SchemaVersionMismatch);
        assert!(e.to_string().contains("v4"), "{e}");
        assert_eq!(stamp_of(&pool).await, 0, "nothing was stamped");
    }

    #[tokio::test]
    async fn a_rung_that_fails_leaves_the_stamp_where_it_was() {
        let pool = raw().await;
        sqlx::query("PRAGMA user_version = 1")
            .execute(&pool)
            .await
            .unwrap();
        let ladder: &[(i64, &str)] =
            &[(1, "CREATE TABLE ok (x INTEGER);"), (2, "THIS IS NOT SQL;")];
        assert!(climb(&pool, 1, 3, ladder).await.is_err());
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
        assert!(e.to_string().contains("Upgrade gmr"), "{e}");
    }

    #[tokio::test]
    async fn a_database_already_at_this_version_is_left_alone() {
        let store = open_in_memory().await.unwrap();
        let before = shape(store.pool()).await;
        migrate(store.pool()).await.unwrap();
        assert_eq!(before, shape(store.pool()).await);
        assert_eq!(stamp_of(store.pool()).await, schema::SCHEMA_VERSION);
    }

    const V6_SETTINGS: &str = "CREATE TABLE settings (\
        anchor TEXT PRIMARY KEY, retain TEXT NOT NULL, cadence_secs INTEGER);";

    #[tokio::test]
    async fn a_real_v6_database_is_carried_to_v7_with_what_it_held() {
        let pool = raw().await;
        sqlx::raw_sql(V6_SETTINGS).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO settings VALUES ('a#b', 'full', 900)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA user_version = 6")
            .execute(&pool)
            .await
            .unwrap();

        climb(&pool, 6, 7, LADDER).await.unwrap();

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
    async fn the_settings_table_a_v6_climbs_into_is_the_one_a_fresh_build_makes() {
        let climbed = raw().await;
        sqlx::raw_sql(V6_SETTINGS).execute(&climbed).await.unwrap();
        sqlx::query("PRAGMA user_version = 6")
            .execute(&climbed)
            .await
            .unwrap();
        climb(&climbed, 6, 7, LADDER).await.unwrap();

        let fresh = open_in_memory().await.unwrap();
        assert_eq!(
            columns(&climbed, "settings").await,
            columns(fresh.pool(), "settings").await,
            "the ladder and the full schema are two descriptions of one shape, and only \
             this comparison keeps them saying the same thing"
        );
    }

    async fn columns(pool: &SqlitePool, table: &str) -> Vec<(String, String)> {
        sqlx::query_as(&format!(
            "SELECT name, type FROM pragma_table_info('{table}') ORDER BY name"
        ))
        .fetch_all(pool)
        .await
        .unwrap()
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

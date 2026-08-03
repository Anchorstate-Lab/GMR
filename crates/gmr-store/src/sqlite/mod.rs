pub mod bindings;
pub mod journal;
pub mod links;
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
pub use queue::SqliteQueue;

pub(crate) fn ref_key(r: &Ref) -> String {
    String::from_utf8(canonicalize(
        &serde_json::to_value(r).expect("a Ref always serialises"),
    ))
    .expect("canonical JSON is always UTF-8")
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
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(db_err)?;
    migrate(&pool).await?;
    Ok(SqliteStore { pool })
}

async fn migrate(pool: &SqlitePool) -> Result<(), StoreError> {
    let stamped: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

    if stamped != 0 && stamped != schema::SCHEMA_VERSION {
        return Err(StoreError::with_code(
            ErrorKind::Constraint,
            ErrorCode::SchemaVersionMismatch,
            format!(
                "this database is stamped schema v{stamped}, this generation is v{}. \
             Refusing to open — misreading a database from another generation is \
             far worse than not opening it",
                schema::SCHEMA_VERSION
            ),
        ));
    }

    sqlx::raw_sql(schema::SCHEMA)
        .execute(pool)
        .await
        .map_err(db_err)?;

    if stamped == 0 {
        sqlx::query(&format!("PRAGMA user_version = {}", schema::SCHEMA_VERSION))
            .execute(pool)
            .await
            .map_err(db_err)?;
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

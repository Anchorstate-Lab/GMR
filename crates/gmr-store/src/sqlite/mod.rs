pub mod bindings;
pub mod journal;
pub mod queue;
pub mod schema;

use std::path::Path;

use crate::{ErrorKind, StoreError};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

pub use bindings::SqliteBindings;
pub use journal::SqliteJournal;
pub use queue::SqliteQueue;

pub(crate) fn db_err(e: sqlx::Error) -> StoreError {
    let kind = match &e {
        sqlx::Error::Database(_) => ErrorKind::Constraint,
        sqlx::Error::Io(_) => ErrorKind::Io,
        sqlx::Error::PoolTimedOut => ErrorKind::Busy,
        _ => ErrorKind::Other,
    };
    StoreError::new(kind, e.to_string())
}

pub(crate) fn decode_err(e: serde_json::Error) -> StoreError {
    StoreError::corrupt(format!("存着的字节不是它该是的样子：{e}"))
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
        return Err(StoreError::constraint(format!(
            "这个库盖的是 schema v{stamped}，本代是 v{}。\
             拒绝打开 —— 误读一个另一代的库，比打不开坏得多",
            schema::SCHEMA_VERSION
        )));
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

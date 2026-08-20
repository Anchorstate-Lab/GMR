use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::pool::PoolConnection;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row as _, SqlitePool};

use crate::index::{Built, Fault, Generation, Index, IndexError, Indexed, Located, Row, Snapshot};
use crate::matching::Want;
use crate::walk::{Held, Stamp};

pub const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS generation (
    id        TEXT PRIMARY KEY,
    probe     TEXT NOT NULL,
    version   TEXT NOT NULL,
    sealed_at INTEGER
);

CREATE TABLE IF NOT EXISTS file (
    generation TEXT NOT NULL,
    rel        TEXT NOT NULL,
    hash       TEXT NOT NULL,
    sort       TEXT NOT NULL,
    mtime_ns   INTEGER,
    size       INTEGER,
    PRIMARY KEY (generation, rel)
);

CREATE TABLE IF NOT EXISTS candidate (
    generation TEXT NOT NULL,
    rel        TEXT NOT NULL,
    ord        INTEGER NOT NULL,
    id         TEXT NOT NULL,
    coord      TEXT NOT NULL,
    facts      TEXT NOT NULL,
    PRIMARY KEY (generation, rel, ord)
);

CREATE TABLE IF NOT EXISTS posting (
    generation TEXT NOT NULL,
    item       TEXT NOT NULL,
    value      TEXT NOT NULL,
    rel        TEXT NOT NULL,
    ord        INTEGER NOT NULL,
    PRIMARY KEY (generation, item, value, rel, ord)
);
"#;

const KINDS: [&str; 4] = ["view", "trigger", "index", "table"];

const MARKER: &str = "generation";

fn db_err(e: sqlx::Error) -> IndexError {
    let fault = match &e {
        sqlx::Error::Database(db) if db.message().contains("locked") => Fault::Busy,
        sqlx::Error::Database(_) => Fault::Corrupt,
        sqlx::Error::Io(_) => Fault::Io,
        _ => Fault::Other,
    };
    IndexError::new(fault, e.to_string())
}

fn unreadable(what: &str, e: serde_json::Error) -> IndexError {
    IndexError::new(
        Fault::Corrupt,
        format!("a stored {what} is not the JSON this build wrote ({e})"),
    )
}

#[derive(Debug)]
pub struct SqliteIndex {
    pool: SqlitePool,
}

impl SqliteIndex {
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

pub async fn open(file: impl AsRef<Path>) -> Result<SqliteIndex, IndexError> {
    let file = file.as_ref();
    let options = SqliteConnectOptions::new()
        .filename(file)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(db_err)?;
    ready(&pool, &file.display().to_string()).await?;
    Ok(SqliteIndex { pool })
}

pub async fn open_in_memory() -> Result<SqliteIndex, IndexError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(SqliteConnectOptions::new().in_memory(true))
        .await
        .map_err(db_err)?;
    ready(&pool, "an in-memory index").await?;
    Ok(SqliteIndex { pool })
}

async fn ready(pool: &SqlitePool, what: &str) -> Result<(), IndexError> {
    let stamped: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
    if stamped == SCHEMA_VERSION {
        return Ok(());
    }
    let mut held = pool.acquire().await.map_err(db_err)?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *held)
        .await
        .map_err(db_err)?;
    let raised = raise(&mut held, what).await;
    let closed = sqlx::query(match raised.is_ok() {
        true => "COMMIT",
        false => "ROLLBACK",
    })
    .execute(&mut *held)
    .await;
    raised?;
    closed.map_err(db_err)?;
    Ok(())
}

async fn raise(held: &mut PoolConnection<sqlx::Sqlite>, what: &str) -> Result<(), IndexError> {
    let stamped: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut **held)
        .await
        .map_err(db_err)?;
    if stamped == SCHEMA_VERSION {
        return Ok(());
    }
    let holds = strangers(held).await?;
    if !holds.is_empty() {
        return Err(IndexError::foreign(what, &holds));
    }
    raze(held).await?;
    sqlx::raw_sql(SCHEMA)
        .execute(&mut **held)
        .await
        .map_err(db_err)?;
    sqlx::query(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
        .execute(&mut **held)
        .await
        .map_err(db_err)?;
    Ok(())
}

async fn strangers(held: &mut PoolConnection<sqlx::Sqlite>) -> Result<Vec<String>, IndexError> {
    let named: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(&mut **held)
    .await
    .map_err(db_err)?;
    match named.iter().any(|name| name == MARKER) {
        true => Ok(Vec::new()),
        false => Ok(named),
    }
}

async fn raze(held: &mut PoolConnection<sqlx::Sqlite>) -> Result<(), IndexError> {
    for kind in KINDS {
        let named: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = ? AND name NOT LIKE 'sqlite_%'",
        )
        .bind(kind)
        .fetch_all(&mut **held)
        .await
        .map_err(db_err)?;
        for name in named {
            let quoted = name.replace('"', "\"\"");
            sqlx::query(&format!("DROP {kind} IF EXISTS \"{quoted}\""))
                .execute(&mut **held)
                .await
                .map_err(db_err)?;
        }
    }
    Ok(())
}

fn beneath(sql: &mut String, root: &str) -> bool {
    match root {
        "" | "." => false,
        _ => {
            sql.push_str(" AND substr(f.rel, 1, ?) = ? AND substr(f.rel, ? + 1, 1) = '/'");
            true
        }
    }
}

fn found(row: &sqlx::sqlite::SqliteRow) -> Result<Located, IndexError> {
    let coord: String = row.get("coord");
    let facts: String = row.get("facts");
    Ok(Located {
        rel: row.get("rel"),
        row: Row {
            ord: row.get::<i64, _>("ord") as u32,
            id: row.get("id"),
            coord: serde_json::from_str(&coord).map_err(|e| unreadable("coordinate", e))?,
            facts: serde_json::from_str(&facts).map_err(|e| unreadable("fact", e))?,
        },
    })
}

fn stamped(row: &sqlx::sqlite::SqliteRow) -> Option<DateTime<Utc>> {
    row.get::<Option<i64>, _>("sealed_at")
        .and_then(|s| DateTime::from_timestamp(s, 0))
}

async fn opened(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    of: &Generation,
) -> Result<Option<Snapshot>, IndexError> {
    let row = sqlx::query("SELECT sealed_at FROM generation WHERE id = ?")
        .bind(of.as_str())
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_err)?;
    Ok(row.map(|r| Snapshot {
        sealed_at: stamped(&r),
        rows: Vec::new(),
    }))
}

const READ: &str = "SELECT f.sort, f.rel, c.ord, c.id, c.coord, c.facts \
     FROM candidate c JOIN file f ON f.generation = c.generation AND f.rel = c.rel \
     WHERE c.generation = ?";

const TAIL: &str = " ORDER BY f.sort, c.ord";

#[async_trait]
impl Index for SqliteIndex {
    async fn built(&self, of: &Generation) -> Result<Option<Built>, IndexError> {
        let row = sqlx::query(
            "SELECT sealed_at, \
             (SELECT COUNT(*) FROM file WHERE generation = g.id) AS files, \
             (SELECT COUNT(*) FROM candidate WHERE generation = g.id) AS rows \
             FROM generation g WHERE g.id = ?",
        )
        .bind(of.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(row.map(|r| Built {
            files: r.get::<i64, _>("files") as u64,
            rows: r.get::<i64, _>("rows") as u64,
            sealed_at: stamped(&r),
        }))
    }

    async fn known(&self, of: &Generation) -> Result<BTreeMap<String, Held>, IndexError> {
        let rows = sqlx::query("SELECT rel, hash, mtime_ns, size FROM file WHERE generation = ?")
            .bind(of.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(rows
            .iter()
            .map(|r| {
                let stamp = match (
                    r.get::<Option<i64>, _>("mtime_ns"),
                    r.get::<Option<i64>, _>("size"),
                ) {
                    (Some(mtime_ns), Some(size)) => Some(Stamp {
                        mtime_ns,
                        size: size as u64,
                    }),
                    _ => None,
                };
                (
                    r.get("rel"),
                    Held {
                        hash: r.get("hash"),
                        stamp,
                    },
                )
            })
            .collect())
    }

    async fn write(&self, of: &Generation, files: &[Indexed]) -> Result<(), IndexError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;

        sqlx::query(
            "INSERT INTO generation (id, probe, version, sealed_at) VALUES (?, ?, ?, NULL) \
             ON CONFLICT(id) DO UPDATE SET sealed_at = NULL",
        )
        .bind(of.as_str())
        .bind(of.probe())
        .bind(of.version())
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;

        for file in files {
            for table in ["posting", "candidate"] {
                sqlx::query(&format!(
                    "DELETE FROM {table} WHERE generation = ? AND rel = ?"
                ))
                .bind(of.as_str())
                .bind(&file.rel)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
            }

            sqlx::query(
                "INSERT INTO file (generation, rel, hash, sort, mtime_ns, size) \
                 VALUES (?, ?, ?, ?, ?, ?) \
                 ON CONFLICT(generation, rel) DO UPDATE SET hash = excluded.hash, \
                 sort = excluded.sort, mtime_ns = excluded.mtime_ns, size = excluded.size",
            )
            .bind(of.as_str())
            .bind(&file.rel)
            .bind(&file.hash)
            .bind(&file.sort)
            .bind(file.stamp.map(|s| s.mtime_ns))
            .bind(file.stamp.map(|s| s.size as i64))
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;

            for row in &file.rows {
                let coord = serde_json::to_string(&row.coord)
                    .map_err(|e| IndexError::new(Fault::Other, e.to_string()))?;
                let facts = serde_json::to_string(&row.facts)
                    .map_err(|e| IndexError::new(Fault::Other, e.to_string()))?;
                sqlx::query(
                    "INSERT INTO candidate (generation, rel, ord, id, coord, facts) \
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(of.as_str())
                .bind(&file.rel)
                .bind(row.ord as i64)
                .bind(&row.id)
                .bind(&coord)
                .bind(&facts)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;

                for (item, value) in &row.coord {
                    sqlx::query(
                        "INSERT INTO posting (generation, item, value, rel, ord) \
                         VALUES (?, ?, ?, ?, ?)",
                    )
                    .bind(of.as_str())
                    .bind(item)
                    .bind(value)
                    .bind(&file.rel)
                    .bind(row.ord as i64)
                    .execute(&mut *tx)
                    .await
                    .map_err(db_err)?;
                }
            }
        }

        tx.commit().await.map_err(db_err)
    }

    async fn restamp(
        &self,
        of: &Generation,
        restamped: &[(String, Option<Stamp>)],
    ) -> Result<(), IndexError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        for (rel, stamp) in restamped {
            sqlx::query("UPDATE file SET mtime_ns = ?, size = ? WHERE generation = ? AND rel = ?")
                .bind(stamp.map(|s| s.mtime_ns))
                .bind(stamp.map(|s| s.size as i64))
                .bind(of.as_str())
                .bind(rel)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn forget(&self, of: &Generation, gone: &[String]) -> Result<(), IndexError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        for rel in gone {
            for table in ["posting", "candidate", "file"] {
                sqlx::query(&format!(
                    "DELETE FROM {table} WHERE generation = ? AND rel = ?"
                ))
                .bind(of.as_str())
                .bind(rel)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
            }
        }
        tx.commit().await.map_err(db_err)
    }

    async fn seal(&self, of: &Generation, at: DateTime<Utc>) -> Result<(), IndexError> {
        let done = sqlx::query("UPDATE generation SET sealed_at = ? WHERE id = ?")
            .bind(at.timestamp())
            .bind(of.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        match done.rows_affected() {
            0 => Err(IndexError::unopened(of)),
            _ => Ok(()),
        }
    }

    async fn generations(&self) -> Result<Vec<(Generation, Built)>, IndexError> {
        let rows = sqlx::query(
            "SELECT probe, version, sealed_at, \
             (SELECT COUNT(*) FROM file WHERE generation = g.id) AS files, \
             (SELECT COUNT(*) FROM candidate WHERE generation = g.id) AS rows \
             FROM generation g ORDER BY probe, version",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(rows
            .iter()
            .map(|r| {
                (
                    Generation::of(
                        r.get::<String, _>("probe").as_str(),
                        r.get::<String, _>("version").as_str(),
                    ),
                    Built {
                        files: r.get::<i64, _>("files") as u64,
                        rows: r.get::<i64, _>("rows") as u64,
                        sealed_at: stamped(r),
                    },
                )
            })
            .collect())
    }

    async fn discard(&self, of: &Generation) -> Result<(), IndexError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        for table in ["posting", "candidate", "file"] {
            sqlx::query(&format!("DELETE FROM {table} WHERE generation = ?"))
                .bind(of.as_str())
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        sqlx::query("DELETE FROM generation WHERE id = ?")
            .bind(of.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn rows(&self, of: &Generation, root: &str) -> Result<Option<Snapshot>, IndexError> {
        let mut sql = String::from(READ);
        let narrowed = beneath(&mut sql, root);
        sql.push_str(TAIL);

        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let Some(snapshot) = opened(&mut tx, of).await? else {
            return Ok(None);
        };
        let mut query = sqlx::query(&sql).bind(of.as_str());
        if narrowed {
            query = query
                .bind(root.len() as i64)
                .bind(root)
                .bind(root.len() as i64);
        }
        let rows = query.fetch_all(&mut *tx).await.map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(Some(Snapshot {
            rows: rows.iter().map(found).collect::<Result<_, _>>()?,
            ..snapshot
        }))
    }

    async fn union(
        &self,
        of: &Generation,
        root: &str,
        want: &Want,
    ) -> Result<Option<Snapshot>, IndexError> {
        let mut sql = String::from(
            "SELECT DISTINCT f.sort, f.rel, c.ord, c.id, c.coord, c.facts \
             FROM posting p \
             JOIN candidate c ON c.generation = p.generation AND c.rel = p.rel \
             AND c.ord = p.ord \
             JOIN file f ON f.generation = c.generation AND f.rel = c.rel \
             WHERE p.generation = ?",
        );
        let narrowed = beneath(&mut sql, root);
        let pairs: Vec<&str> = want
            .iter()
            .map(|_| "(p.item = ? AND p.value = ?)")
            .collect();
        sql.push_str(&format!(" AND ({})", pairs.join(" OR ")));
        sql.push_str(TAIL);

        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let Some(snapshot) = opened(&mut tx, of).await? else {
            return Ok(None);
        };
        let rows = match want.is_empty() {
            true => Vec::new(),
            false => {
                let mut query = sqlx::query(&sql).bind(of.as_str());
                if narrowed {
                    query = query
                        .bind(root.len() as i64)
                        .bind(root)
                        .bind(root.len() as i64);
                }
                for (item, value) in want {
                    query = query.bind(item).bind(value);
                }
                query.fetch_all(&mut *tx).await.map_err(db_err)?
            }
        };
        tx.commit().await.map_err(db_err)?;
        Ok(Some(Snapshot {
            rows: rows.iter().map(found).collect::<Result<_, _>>()?,
            ..snapshot
        }))
    }
}

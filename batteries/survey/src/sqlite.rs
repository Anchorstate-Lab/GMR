use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

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

const LOCK_POISONED: &str = "gmr-survey: a prior SQLite call panicked while holding the lock";

fn db_err(e: rusqlite::Error) -> IndexError {
    let fault = match &e {
        rusqlite::Error::SqliteFailure(err, _) => match err.code {
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => Fault::Busy,
            _ => Fault::Corrupt,
        },
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
    conn: Mutex<Connection>,
}

impl SqliteIndex {
    pub async fn close(&self) {}

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect(LOCK_POISONED)
    }
}

pub async fn open(file: impl AsRef<Path>) -> Result<SqliteIndex, IndexError> {
    let file = file.as_ref();
    let mut conn = Connection::open(file).map_err(db_err)?;
    conn.execute_batch("PRAGMA journal_mode = WAL")
        .map_err(db_err)?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(db_err)?;
    ready(&mut conn, &file.display().to_string())?;
    Ok(SqliteIndex {
        conn: Mutex::new(conn),
    })
}

pub async fn open_in_memory() -> Result<SqliteIndex, IndexError> {
    let mut conn = Connection::open_in_memory().map_err(db_err)?;
    ready(&mut conn, "an in-memory index")?;
    Ok(SqliteIndex {
        conn: Mutex::new(conn),
    })
}

fn ready(conn: &mut Connection, what: &str) -> Result<(), IndexError> {
    let stamped: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(db_err)?;
    if stamped == SCHEMA_VERSION {
        return Ok(());
    }
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(db_err)?;
    raise(&tx, what)?;
    tx.commit().map_err(db_err)
}

fn raise(tx: &Transaction, what: &str) -> Result<(), IndexError> {
    let stamped: i64 = tx
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(db_err)?;
    if stamped == SCHEMA_VERSION {
        return Ok(());
    }
    let holds = strangers(tx)?;
    if !holds.is_empty() {
        return Err(IndexError::foreign(what, &holds));
    }
    raze(tx)?;
    tx.execute_batch(SCHEMA).map_err(db_err)?;
    tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
        .map_err(db_err)?;
    Ok(())
}

fn strangers(tx: &Transaction) -> Result<Vec<String>, IndexError> {
    let mut stmt = tx
        .prepare("SELECT name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY name")
        .map_err(db_err)?;
    let named: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(db_err)?
        .collect::<Result<_, _>>()
        .map_err(db_err)?;
    match named.iter().any(|name| name == MARKER) {
        true => Ok(Vec::new()),
        false => Ok(named),
    }
}

fn raze(tx: &Transaction) -> Result<(), IndexError> {
    for kind in KINDS {
        let named: Vec<String> = {
            let mut stmt = tx
                .prepare(
                    "SELECT name FROM sqlite_master WHERE type = ? AND name NOT LIKE 'sqlite_%'",
                )
                .map_err(db_err)?;
            stmt.query_map([kind], |r| r.get(0))
                .map_err(db_err)?
                .collect::<Result<_, _>>()
                .map_err(db_err)?
        };
        for name in named {
            let quoted = name.replace('"', "\"\"");
            tx.execute_batch(&format!("DROP {kind} IF EXISTS \"{quoted}\""))
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

struct RawFound {
    sort: String,
    rel: String,
    ord: i64,
    id: String,
    coord: String,
    facts: String,
}

fn raw_found(row: &rusqlite::Row) -> rusqlite::Result<RawFound> {
    Ok(RawFound {
        sort: row.get("sort")?,
        rel: row.get("rel")?,
        ord: row.get("ord")?,
        id: row.get("id")?,
        coord: row.get("coord")?,
        facts: row.get("facts")?,
    })
}

fn decode(raw: RawFound) -> Result<Located, IndexError> {
    Ok(Located {
        rel: raw.rel,
        row: Row {
            ord: raw.ord as u32,
            id: raw.id,
            coord: serde_json::from_str(&raw.coord).map_err(|e| unreadable("coordinate", e))?,
            facts: serde_json::from_str(&raw.facts).map_err(|e| unreadable("fact", e))?,
        },
    })
}

fn stamped(row: &rusqlite::Row) -> Option<DateTime<Utc>> {
    row.get::<_, Option<i64>>("sealed_at")
        .ok()
        .flatten()
        .and_then(|s| DateTime::from_timestamp(s, 0))
}

fn opened(tx: &Transaction, of: &Generation) -> Result<Option<Snapshot>, IndexError> {
    tx.query_row(
        "SELECT sealed_at FROM generation WHERE id = ?",
        [of.as_str()],
        |r| {
            Ok(Snapshot {
                sealed_at: stamped(r),
                rows: Vec::new(),
            })
        },
    )
    .optional()
    .map_err(db_err)
}

fn locate(tx: &Transaction, sql: &str, params: &[SqlValue]) -> Result<Vec<RawFound>, IndexError> {
    let mut stmt = tx.prepare_cached(sql).map_err(db_err)?;
    stmt.query_map(rusqlite::params_from_iter(params.iter()), raw_found)
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)
}

const READ: &str = "SELECT f.sort, f.rel, c.ord, c.id, c.coord, c.facts \
     FROM candidate c JOIN file f ON f.generation = c.generation AND f.rel = c.rel \
     WHERE c.generation = ?";

const TAIL: &str = " ORDER BY f.sort, c.ord";

const POINT: &str = "SELECT f.sort, f.rel, c.ord, c.id, c.coord, c.facts \
     FROM posting p \
     JOIN candidate c ON c.generation = p.generation AND c.rel = p.rel AND c.ord = p.ord \
     JOIN file f ON f.generation = c.generation AND f.rel = c.rel \
     WHERE p.generation = ? AND p.item = ? AND p.value = ?";

#[async_trait]
impl Index for SqliteIndex {
    async fn built(&self, of: &Generation) -> Result<Option<Built>, IndexError> {
        let conn = self.conn.lock().expect(LOCK_POISONED);
        conn.query_row(
            "SELECT sealed_at, \
             (SELECT COUNT(*) FROM file WHERE generation = g.id) AS files, \
             (SELECT COUNT(*) FROM candidate WHERE generation = g.id) AS rows \
             FROM generation g WHERE g.id = ?",
            [of.as_str()],
            |r| {
                Ok(Built {
                    files: r.get::<_, i64>("files")? as u64,
                    rows: r.get::<_, i64>("rows")? as u64,
                    sealed_at: stamped(r),
                })
            },
        )
        .optional()
        .map_err(db_err)
    }

    async fn known(&self, of: &Generation) -> Result<BTreeMap<String, Held>, IndexError> {
        let conn = self.conn.lock().expect(LOCK_POISONED);
        let mut stmt = conn
            .prepare("SELECT rel, hash, mtime_ns, size FROM file WHERE generation = ?")
            .map_err(db_err)?;
        stmt.query_map([of.as_str()], |r| {
            let stamp = match (
                r.get::<_, Option<i64>>("mtime_ns")?,
                r.get::<_, Option<i64>>("size")?,
            ) {
                (Some(mtime_ns), Some(size)) => Some(Stamp {
                    mtime_ns,
                    size: size as u64,
                }),
                _ => None,
            };
            Ok((
                r.get::<_, String>("rel")?,
                Held {
                    hash: r.get("hash")?,
                    stamp,
                },
            ))
        })
        .map_err(db_err)?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(db_err)
    }

    async fn write(&self, of: &Generation, files: &[Indexed]) -> Result<(), IndexError> {
        let mut conn = self.conn.lock().expect(LOCK_POISONED);
        let tx = conn.transaction().map_err(db_err)?;

        tx.execute(
            "INSERT INTO generation (id, probe, version, sealed_at) VALUES (?, ?, ?, NULL) \
             ON CONFLICT(id) DO UPDATE SET sealed_at = NULL",
            params![of.as_str(), of.probe(), of.version()],
        )
        .map_err(db_err)?;

        {
            let mut del_posting = tx
                .prepare_cached("DELETE FROM posting WHERE generation = ? AND rel = ?")
                .map_err(db_err)?;
            let mut del_candidate = tx
                .prepare_cached("DELETE FROM candidate WHERE generation = ? AND rel = ?")
                .map_err(db_err)?;
            let mut put_file = tx
                .prepare_cached(
                    "INSERT INTO file (generation, rel, hash, sort, mtime_ns, size) \
                     VALUES (?, ?, ?, ?, ?, ?) \
                     ON CONFLICT(generation, rel) DO UPDATE SET hash = excluded.hash, \
                     sort = excluded.sort, mtime_ns = excluded.mtime_ns, size = excluded.size",
                )
                .map_err(db_err)?;
            let mut put_candidate = tx
                .prepare_cached(
                    "INSERT INTO candidate (generation, rel, ord, id, coord, facts) \
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .map_err(db_err)?;
            let mut put_posting = tx
                .prepare_cached(
                    "INSERT INTO posting (generation, item, value, rel, ord) VALUES (?, ?, ?, ?, ?)",
                )
                .map_err(db_err)?;

            for file in files {
                del_posting
                    .execute(params![of.as_str(), &file.rel])
                    .map_err(db_err)?;
                del_candidate
                    .execute(params![of.as_str(), &file.rel])
                    .map_err(db_err)?;
                put_file
                    .execute(params![
                        of.as_str(),
                        &file.rel,
                        &file.hash,
                        &file.sort,
                        file.stamp.map(|s| s.mtime_ns),
                        file.stamp.map(|s| s.size as i64),
                    ])
                    .map_err(db_err)?;

                for row in &file.rows {
                    let coord = serde_json::to_string(&row.coord)
                        .map_err(|e| IndexError::new(Fault::Other, e.to_string()))?;
                    let facts = serde_json::to_string(&row.facts)
                        .map_err(|e| IndexError::new(Fault::Other, e.to_string()))?;
                    put_candidate
                        .execute(params![
                            of.as_str(),
                            &file.rel,
                            row.ord as i64,
                            &row.id,
                            &coord,
                            &facts
                        ])
                        .map_err(db_err)?;
                    for (item, value) in &row.coord {
                        put_posting
                            .execute(params![of.as_str(), item, value, &file.rel, row.ord as i64])
                            .map_err(db_err)?;
                    }
                }
            }
        }

        tx.commit().map_err(db_err)
    }

    async fn restamp(
        &self,
        of: &Generation,
        restamped: &[(String, Option<Stamp>)],
    ) -> Result<(), IndexError> {
        let mut conn = self.conn.lock().expect(LOCK_POISONED);
        let tx = conn.transaction().map_err(db_err)?;
        {
            let mut stmt = tx
                .prepare_cached(
                    "UPDATE file SET mtime_ns = ?, size = ? WHERE generation = ? AND rel = ?",
                )
                .map_err(db_err)?;
            for (rel, stamp) in restamped {
                stmt.execute(params![
                    stamp.map(|s| s.mtime_ns),
                    stamp.map(|s| s.size as i64),
                    of.as_str(),
                    rel,
                ])
                .map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)
    }

    async fn forget(&self, of: &Generation, gone: &[String]) -> Result<(), IndexError> {
        let mut conn = self.conn.lock().expect(LOCK_POISONED);
        let tx = conn.transaction().map_err(db_err)?;
        for table in ["posting", "candidate", "file"] {
            let mut stmt = tx
                .prepare_cached(&format!(
                    "DELETE FROM {table} WHERE generation = ? AND rel = ?"
                ))
                .map_err(db_err)?;
            for rel in gone {
                stmt.execute(params![of.as_str(), rel]).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)
    }

    async fn seal(&self, of: &Generation, at: DateTime<Utc>) -> Result<(), IndexError> {
        let conn = self.conn.lock().expect(LOCK_POISONED);
        let affected = conn
            .execute(
                "UPDATE generation SET sealed_at = ? WHERE id = ?",
                params![at.timestamp(), of.as_str()],
            )
            .map_err(db_err)?;
        match affected {
            0 => Err(IndexError::unopened(of)),
            _ => Ok(()),
        }
    }

    async fn generations(&self) -> Result<Vec<(Generation, Built)>, IndexError> {
        let conn = self.conn.lock().expect(LOCK_POISONED);
        let mut stmt = conn
            .prepare(
                "SELECT probe, version, sealed_at, \
                 (SELECT COUNT(*) FROM file WHERE generation = g.id) AS files, \
                 (SELECT COUNT(*) FROM candidate WHERE generation = g.id) AS rows \
                 FROM generation g ORDER BY probe, version",
            )
            .map_err(db_err)?;
        stmt.query_map([], |r| {
            Ok((
                Generation::of(
                    &r.get::<_, String>("probe")?,
                    &r.get::<_, String>("version")?,
                ),
                Built {
                    files: r.get::<_, i64>("files")? as u64,
                    rows: r.get::<_, i64>("rows")? as u64,
                    sealed_at: stamped(r),
                },
            ))
        })
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)
    }

    async fn discard(&self, of: &Generation) -> Result<(), IndexError> {
        let mut conn = self.conn.lock().expect(LOCK_POISONED);
        let tx = conn.transaction().map_err(db_err)?;
        for table in ["posting", "candidate", "file"] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE generation = ?"),
                params![of.as_str()],
            )
            .map_err(db_err)?;
        }
        tx.execute("DELETE FROM generation WHERE id = ?", params![of.as_str()])
            .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    async fn rows(&self, of: &Generation, root: &str) -> Result<Option<Snapshot>, IndexError> {
        let mut sql = String::from(READ);
        let narrowed = beneath(&mut sql, root);
        sql.push_str(TAIL);

        let mut conn = self.conn.lock().expect(LOCK_POISONED);
        let tx = conn.transaction().map_err(db_err)?;
        let Some(snapshot) = opened(&tx, of)? else {
            return Ok(None);
        };

        let mut bound = vec![SqlValue::Text(of.as_str().to_owned())];
        if narrowed {
            let root_len = root.len() as i64;
            bound.push(SqlValue::Integer(root_len));
            bound.push(SqlValue::Text(root.to_owned()));
            bound.push(SqlValue::Integer(root_len));
        }
        let raw = locate(&tx, &sql, &bound)?;
        tx.commit().map_err(db_err)?;
        Ok(Some(Snapshot {
            rows: raw.into_iter().map(decode).collect::<Result<_, _>>()?,
            ..snapshot
        }))
    }

    async fn union(
        &self,
        of: &Generation,
        root: &str,
        want: &Want,
    ) -> Result<Option<Snapshot>, IndexError> {
        let mut sql = String::from(POINT);
        let narrowed = beneath(&mut sql, root);

        let mut conn = self.conn.lock().expect(LOCK_POISONED);
        let tx = conn.transaction().map_err(db_err)?;
        let Some(snapshot) = opened(&tx, of)? else {
            return Ok(None);
        };

        let mut merged: BTreeMap<(String, i64), Located> = BTreeMap::new();
        for (item, value) in want {
            let mut bound = vec![
                SqlValue::Text(of.as_str().to_owned()),
                SqlValue::Text(item.clone()),
                SqlValue::Text(value.clone()),
            ];
            if narrowed {
                let root_len = root.len() as i64;
                bound.push(SqlValue::Integer(root_len));
                bound.push(SqlValue::Text(root.to_owned()));
                bound.push(SqlValue::Integer(root_len));
            }
            for raw in locate(&tx, &sql, &bound)? {
                let at = (raw.sort.clone(), raw.ord);
                if !merged.contains_key(&at) {
                    merged.insert(at, decode(raw)?);
                }
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(Some(Snapshot {
            rows: merged.into_values().collect(),
            ..snapshot
        }))
    }
}

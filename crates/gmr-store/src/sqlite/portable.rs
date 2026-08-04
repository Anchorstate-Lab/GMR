//! Escape hatch across `schema::SCHEMA_VERSION` bumps.
//!
//! `migrate()` refuses to open a database stamped with a different schema
//! version than this build — misreading one is worse than not opening it.
//! That refusal has no counterpart: nothing lets the journal, which cannot be
//! rebuilt from anything else, cross the boundary it just closed. Run
//! `export_jsonl` with the *old* binary before upgrading; `import_jsonl` on
//! the *new* binary replays it into a fresh store.
//!
//! `settings` and `queue` are deliberately not carried: they say how an
//! anchor is run, not what it judged, so a plain `sync` reconstructs them —
//! see `RunSettings`'s own doc comment for why that split exists.
//!
//! Row bodies stay `serde_json::Value` rather than the typed `Entry` /
//! `Binding` from gmr-core. Round-tripping through the strict type would
//! defeat the point: a file the old binary writes has to stay readable by a
//! new binary whose `Entry` enum may have grown a variant since.

use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};
use sqlx::Row;

use super::{db_err, decode_err};
use crate::error::StoreError;
use crate::sqlite::SqliteStore;

/// The export format's own identity — independent of `schema::SCHEMA_VERSION`.
/// Bump this only if a row shape below changes, not when the SQL schema does.
pub const EXPORT_SCHEMA: &str = "gmr.store-export.v1";

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortableSummary {
    pub journal: usize,
    pub bindings: usize,
    pub binding_anchors: usize,
    pub links: usize,
    pub sealed: usize,
}

/// One line of the export, tagged so a single stream can carry all five
/// tables plus the manifest without a second file format.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "table", rename_all = "snake_case")]
enum Line {
    Manifest {
        schema: String,
        exported_at: chrono::DateTime<chrono::Utc>,
    },
    Journal {
        seq: i64,
        anchor: String,
        body: serde_json::Value,
    },
    Bindings {
        seq: i64,
        reference: String,
        body: serde_json::Value,
        bound_version: String,
        bound_at_seq: Option<i64>,
    },
    BindingAnchors {
        seq: i64,
        anchor: String,
    },
    Links {
        seq: i64,
        from_ref: String,
        to_ref: String,
        kind: String,
    },
    Sealed {
        address: String,
        body_hex: String,
    },
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("odd number of hex digits".to_owned());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn write_line(out: &mut impl Write, line: &Line) -> Result<(), StoreError> {
    serde_json::to_writer(&mut *out, line)
        .map_err(|e| StoreError::other(format!("cannot encode an export row: {e}")))?;
    out.write_all(b"\n")
        .map_err(|e| StoreError::io(format!("cannot write the export: {e}")))
}

/// `expected` is what the row held when it was written; `landed` is where it
/// came to rest just now. They can only differ if the target was not
/// actually empty — the pre-flight count check makes that a bug, not a
/// possibility, so this turns it into a loud error instead of a silently
/// misaligned `Still.ref_entry` or `bindings.bound_at_seq`.
fn expect_seq(table: &str, expected: i64, landed: i64) -> Result<(), StoreError> {
    if expected == landed {
        return Ok(());
    }
    Err(StoreError::corrupt(format!(
        "{table} row {expected} landed at seq {landed} instead; import only replays into a truly empty store"
    )))
}

impl SqliteStore {
    /// Snapshot the journal, bindings, links and sealed rationale as JSONL.
    /// Table order is fixed (journal, bindings, binding_anchors, links,
    /// sealed) so a later `import_jsonl` sees every foreign key before the
    /// row that names it.
    pub async fn export_jsonl(&self, out: &mut impl Write) -> Result<PortableSummary, StoreError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let mut summary = PortableSummary::default();

        write_line(
            out,
            &Line::Manifest {
                schema: EXPORT_SCHEMA.to_owned(),
                exported_at: chrono::Utc::now(),
            },
        )?;

        let rows = sqlx::query("SELECT seq, anchor, body FROM journal ORDER BY seq")
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        for r in rows {
            let body: serde_json::Value =
                serde_json::from_str(&r.get::<String, _>("body")).map_err(decode_err)?;
            write_line(
                out,
                &Line::Journal {
                    seq: r.get("seq"),
                    anchor: r.get("anchor"),
                    body,
                },
            )?;
            summary.journal += 1;
        }

        let rows = sqlx::query(
            "SELECT seq, reference, body, bound_version, bound_at_seq FROM bindings ORDER BY seq",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        for r in rows {
            let body: serde_json::Value =
                serde_json::from_str(&r.get::<String, _>("body")).map_err(decode_err)?;
            write_line(
                out,
                &Line::Bindings {
                    seq: r.get("seq"),
                    reference: r.get("reference"),
                    body,
                    bound_version: r.get("bound_version"),
                    bound_at_seq: r.get::<Option<i64>, _>("bound_at_seq"),
                },
            )?;
            summary.bindings += 1;
        }

        let rows = sqlx::query("SELECT seq, anchor FROM binding_anchors ORDER BY seq, anchor")
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        for r in rows {
            write_line(
                out,
                &Line::BindingAnchors {
                    seq: r.get("seq"),
                    anchor: r.get("anchor"),
                },
            )?;
            summary.binding_anchors += 1;
        }

        let rows = sqlx::query("SELECT seq, from_ref, to_ref, kind FROM links ORDER BY seq")
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        for r in rows {
            write_line(
                out,
                &Line::Links {
                    seq: r.get("seq"),
                    from_ref: r.get("from_ref"),
                    to_ref: r.get("to_ref"),
                    kind: r.get("kind"),
                },
            )?;
            summary.links += 1;
        }

        let rows = sqlx::query("SELECT address, body FROM sealed ORDER BY address")
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        for r in rows {
            write_line(
                out,
                &Line::Sealed {
                    address: r.get("address"),
                    body_hex: to_hex(&r.get::<Vec<u8>, _>("body")),
                },
            )?;
            summary.sealed += 1;
        }

        Ok(summary)
    }

    /// Replay a JSONL export. Refuses anything but a store with no journal,
    /// bindings, binding_anchors, links or sealed rows: this recreates
    /// history at the exact seq values it was written at, which only holds
    /// when nothing occupies those seqs yet. Atomic — a bad line anywhere
    /// leaves the store exactly as empty as it started.
    pub async fn import_jsonl(&self, input: impl BufRead) -> Result<PortableSummary, StoreError> {
        for (table, sql) in [
            ("journal", "SELECT COUNT(*) FROM journal"),
            ("bindings", "SELECT COUNT(*) FROM bindings"),
            ("binding_anchors", "SELECT COUNT(*) FROM binding_anchors"),
            ("links", "SELECT COUNT(*) FROM links"),
            ("sealed", "SELECT COUNT(*) FROM sealed"),
        ] {
            let n: i64 = sqlx::query_scalar(sql)
                .fetch_one(&self.pool)
                .await
                .map_err(db_err)?;
            if n > 0 {
                return Err(StoreError::constraint(format!(
                    "this store already has {table} history; import only replays into a fresh store"
                )));
            }
        }

        let mut summary = PortableSummary::default();
        let mut saw_manifest = false;
        let mut tx = self.pool.begin().await.map_err(db_err)?;

        for (n, line) in input.lines().enumerate() {
            let line = line.map_err(|e| StoreError::io(format!("line {}: {e}", n + 1)))?;
            if line.trim().is_empty() {
                continue;
            }
            let row: Line = serde_json::from_str(&line)
                .map_err(|e| StoreError::corrupt(format!("line {}: {e}", n + 1)))?;

            match row {
                Line::Manifest { schema, .. } => {
                    if schema != EXPORT_SCHEMA {
                        return Err(StoreError::constraint(format!(
                            "this file's schema is `{schema}`, this build reads `{EXPORT_SCHEMA}`"
                        )));
                    }
                    saw_manifest = true;
                }
                Line::Journal { seq, anchor, body } => {
                    let body = serde_json::to_string(&body)
                        .map_err(|e| StoreError::other(format!("line {}: {e}", n + 1)))?;
                    let landed: i64 = sqlx::query_scalar(
                        "INSERT INTO journal (anchor, fence, body) VALUES (?1, 0, ?2) RETURNING seq",
                    )
                    .bind(&anchor)
                    .bind(&body)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(db_err)?;
                    expect_seq("journal", seq, landed)?;
                    summary.journal += 1;
                }
                Line::Bindings {
                    seq,
                    reference,
                    body,
                    bound_version,
                    bound_at_seq,
                } => {
                    let body = serde_json::to_string(&body)
                        .map_err(|e| StoreError::other(format!("line {}: {e}", n + 1)))?;
                    let landed: i64 = sqlx::query_scalar(
                        "INSERT INTO bindings (reference, body, bound_version, bound_at_seq) \
                         VALUES (?1, ?2, ?3, ?4) RETURNING seq",
                    )
                    .bind(&reference)
                    .bind(&body)
                    .bind(&bound_version)
                    .bind(bound_at_seq)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(db_err)?;
                    expect_seq("bindings", seq, landed)?;
                    summary.bindings += 1;
                }
                Line::BindingAnchors { seq, anchor } => {
                    // `seq` is trusted as-is: the Bindings arm above already
                    // proved the row it names landed at this exact seq.
                    sqlx::query("INSERT INTO binding_anchors (seq, anchor) VALUES (?1, ?2)")
                        .bind(seq)
                        .bind(&anchor)
                        .execute(&mut *tx)
                        .await
                        .map_err(db_err)?;
                    summary.binding_anchors += 1;
                }
                Line::Links {
                    from_ref,
                    to_ref,
                    kind,
                    ..
                } => {
                    sqlx::query("INSERT INTO links (from_ref, to_ref, kind) VALUES (?1, ?2, ?3)")
                        .bind(&from_ref)
                        .bind(&to_ref)
                        .bind(&kind)
                        .execute(&mut *tx)
                        .await
                        .map_err(db_err)?;
                    summary.links += 1;
                }
                Line::Sealed { address, body_hex } => {
                    let bytes = from_hex(&body_hex).map_err(|e| {
                        StoreError::corrupt(format!("line {}: sealed body is not hex: {e}", n + 1))
                    })?;
                    sqlx::query("INSERT INTO sealed (address, body) VALUES (?1, ?2)")
                        .bind(&address)
                        .bind(&bytes)
                        .execute(&mut *tx)
                        .await
                        .map_err(db_err)?;
                    summary.sealed += 1;
                }
            }
        }

        if !saw_manifest {
            return Err(StoreError::corrupt(
                "no manifest row found — is this a `gmr export` file?".to_owned(),
            ));
        }

        tx.commit().await.map_err(db_err)?;
        Ok(summary)
    }
}

pub const SCHEMA_VERSION: i64 = 2;

pub const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- ── Anchor journal: append-only ─────────────────────────────

CREATE TABLE IF NOT EXISTS journal (
    seq     INTEGER PRIMARY KEY AUTOINCREMENT,
    anchor  TEXT    NOT NULL,
    fence   INTEGER NOT NULL,     -- monotonic epoch; 0 when no lease is configured
    body    TEXT    NOT NULL      -- the entry itself, verbatim
);
CREATE INDEX IF NOT EXISTS journal_by_anchor ON journal(anchor, seq);

-- ── Bindings: append-only. Rebinding appends; current = latest row ──

CREATE TABLE IF NOT EXISTS bindings (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    reference  TEXT NOT NULL,     -- canonical Ref
    body       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS bindings_by_reference ON bindings(reference, seq);

-- Reverse index: which records hang on this anchor. Lives and dies with bindings.
CREATE TABLE IF NOT EXISTS binding_anchors (
    seq        INTEGER NOT NULL REFERENCES bindings(seq),
    anchor     TEXT    NOT NULL,
    PRIMARY KEY (seq, anchor)
);
CREATE INDEX IF NOT EXISTS binding_anchors_by_anchor ON binding_anchors(anchor);

-- ── Sealed records: append-only, content addressed ─────────

CREATE TABLE IF NOT EXISTS sealed (
    address  TEXT PRIMARY KEY,
    body     BLOB NOT NULL
);

-- ── Queue: polling deployments only. **Mutable**, no pretence ──

CREATE TABLE IF NOT EXISTS queue (
    anchor       TEXT    PRIMARY KEY,
    due          INTEGER NOT NULL,
    lease_until  INTEGER NOT NULL DEFAULT 0,
    epoch        INTEGER NOT NULL DEFAULT 0,   -- token high-water: only grows, survives retire
    parked       INTEGER NOT NULL DEFAULT 0    -- retired, but the counter stays
);

-- ── Append-only — by trigger, not by good intentions ────────

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
CREATE TRIGGER IF NOT EXISTS sealed_no_update BEFORE UPDATE ON sealed
    BEGIN SELECT RAISE(ABORT, 'sealed_immutable'); END;
CREATE TRIGGER IF NOT EXISTS sealed_no_delete BEFORE DELETE ON sealed
    BEGIN SELECT RAISE(ABORT, 'sealed_immutable'); END;
"#;

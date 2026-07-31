pub const SCHEMA_VERSION: i64 = 1;

pub const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- ── 锚日志：只增不改 ────────────────────────────────────────

CREATE TABLE IF NOT EXISTS journal (
    seq     INTEGER PRIMARY KEY AUTOINCREMENT,
    anchor  TEXT    NOT NULL,
    fence   INTEGER NOT NULL,     -- 单调 epoch，未配置租约时恒为 0
    body    TEXT    NOT NULL      -- 条目本身，原样
);
CREATE INDEX IF NOT EXISTS journal_by_anchor ON journal(anchor, seq);

-- ── 挂靠：只增不改。重新绑定是追加，当前值 = 最新一行 ──────

CREATE TABLE IF NOT EXISTS bindings (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    reference  TEXT NOT NULL,     -- canonical Ref
    body       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS bindings_by_reference ON bindings(reference, seq);

-- 反查索引：哪些记录挂在这个锚上。跟 bindings 同生同灭。
CREATE TABLE IF NOT EXISTS binding_anchors (
    seq        INTEGER NOT NULL REFERENCES bindings(seq),
    anchor     TEXT    NOT NULL,
    PRIMARY KEY (seq, anchor)
);
CREATE INDEX IF NOT EXISTS binding_anchors_by_anchor ON binding_anchors(anchor);

-- ── 密封记录：只增不改，内容寻址 ────────────────────────────

CREATE TABLE IF NOT EXISTS sealed (
    address  TEXT PRIMARY KEY,
    body     BLOB NOT NULL
);

-- ── 队列：仅轮询部署。**可变**，不伪装 append-only ──────────

CREATE TABLE IF NOT EXISTS queue (
    anchor       TEXT    PRIMARY KEY,
    due          INTEGER NOT NULL,
    lease_until  INTEGER NOT NULL DEFAULT 0,
    epoch        INTEGER NOT NULL DEFAULT 0
);

-- ── 只增不改 —— by trigger, not by good intentions ──────────

CREATE TRIGGER IF NOT EXISTS journal_no_update BEFORE UPDATE ON journal
    BEGIN SELECT RAISE(ABORT, '日志只增不改'); END;
CREATE TRIGGER IF NOT EXISTS journal_no_delete BEFORE DELETE ON journal
    BEGIN SELECT RAISE(ABORT, '日志只增不改'); END;
CREATE TRIGGER IF NOT EXISTS bindings_no_update BEFORE UPDATE ON bindings
    BEGIN SELECT RAISE(ABORT, '挂靠只增不改：重新绑定是追加'); END;
CREATE TRIGGER IF NOT EXISTS bindings_no_delete BEFORE DELETE ON bindings
    BEGIN SELECT RAISE(ABORT, '挂靠只增不改：重新绑定是追加'); END;
CREATE TRIGGER IF NOT EXISTS binding_anchors_no_update BEFORE UPDATE ON binding_anchors
    BEGIN SELECT RAISE(ABORT, '挂靠只增不改'); END;
CREATE TRIGGER IF NOT EXISTS binding_anchors_no_delete BEFORE DELETE ON binding_anchors
    BEGIN SELECT RAISE(ABORT, '挂靠只增不改'); END;
CREATE TRIGGER IF NOT EXISTS sealed_no_update BEFORE UPDATE ON sealed
    BEGIN SELECT RAISE(ABORT, '密封记录不可篡改 —— 这是基底唯一担保的那件事'); END;
CREATE TRIGGER IF NOT EXISTS sealed_no_delete BEFORE DELETE ON sealed
    BEGIN SELECT RAISE(ABORT, '密封记录不可篡改 —— 这是基底唯一担保的那件事'); END;
"#;

pub const SCHEMA_VERSION: i64 = 12;

pub const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- ── Anchor journal: append-only ─────────────────────────────

CREATE TABLE IF NOT EXISTS journal (
    seq     INTEGER PRIMARY KEY AUTOINCREMENT,
    anchor  TEXT    NOT NULL,
    fence   INTEGER NOT NULL,     -- monotonic epoch; 0 when no lease is configured
    body    TEXT    NOT NULL,     -- the entry itself, verbatim
    prev    TEXT,                 -- the hash this row was linked onto; NULL for the first,
                                  -- and for rows a build without the chain wrote
    hash    TEXT                  -- over the canonical form, not over `body`'s bytes: the
                                  -- same entries exported and imported must chain the same
);
CREATE INDEX IF NOT EXISTS journal_by_anchor ON journal(anchor, seq);

-- ── Bindings: append-only. Rebinding appends; current = latest row ──

CREATE TABLE IF NOT EXISTS bindings (
    seq             INTEGER PRIMARY KEY AUTOINCREMENT,
    reference       TEXT NOT NULL,     -- canonical Claim; a stored one spells itself as its Ref
    body            TEXT NOT NULL,     -- the Binding relation itself (claim + anchors)
    bound_version   TEXT,              -- content version this assertion cited; NULL until a
                                       -- fetch has answered for the record even once
    bound_at_seq    INTEGER,           -- the journal's position at bind time; seq is global
                                       -- across anchors, so one number dates a binding to
                                       -- any number of them. NULL only predates this column
    source          TEXT NOT NULL,     -- how this assertion came to be; the domain's word
    asserted_at     TEXT,              -- RFC3339; NULL predates this column
    baseline_at_seq INTEGER,           -- the bindings row whose fetch established bound_version;
                                       -- NULL while it has never been verified
    saw             TEXT               -- the fact address this assertion was made in front of;
                                       -- NULL when the asserter was shown nothing
);
CREATE INDEX IF NOT EXISTS bindings_by_reference ON bindings(reference, seq);

-- Reverse index, and the OR-Set tag space: one tag is one (seq, anchor).
CREATE TABLE IF NOT EXISTS binding_anchors (
    seq        INTEGER NOT NULL REFERENCES bindings(seq),
    anchor     TEXT    NOT NULL,
    PRIMARY KEY (seq, anchor)
);
CREATE INDEX IF NOT EXISTS binding_anchors_by_anchor ON binding_anchors(anchor);

-- Revocations: a claim about specific prior assertions, never a flag on them.
-- `anchor` is the generation the revocation was made at; a read of that
-- generation or a later one sees it, a read of an ancestor does not.
CREATE TABLE IF NOT EXISTS binding_revocations (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    reference   TEXT NOT NULL,
    anchor      TEXT NOT NULL,
    source      TEXT NOT NULL,
    revoked_at  TEXT
);
CREATE INDEX IF NOT EXISTS binding_revocations_by_anchor ON binding_revocations(anchor);

-- The tags a revocation observed and killed. Naming them is what keeps a
-- later add of the same anchor alive: it is a tag this revocation never saw.
CREATE TABLE IF NOT EXISTS binding_revoked_tags (
    revocation  INTEGER NOT NULL REFERENCES binding_revocations(seq),
    binding     INTEGER NOT NULL,
    anchor      TEXT    NOT NULL,
    PRIMARY KEY (revocation, binding, anchor)
);

-- ── Links: Ref -> Ref, a different arity than bindings. Append-only ──

CREATE TABLE IF NOT EXISTS links (
    seq      INTEGER PRIMARY KEY AUTOINCREMENT,
    from_ref TEXT NOT NULL,     -- canonical Ref
    to_ref   TEXT NOT NULL,     -- canonical Ref
    kind     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS links_by_from ON links(from_ref);

-- ── Sealed records: append-only, content addressed ─────────

CREATE TABLE IF NOT EXISTS sealed (
    address  TEXT PRIMARY KEY,
    body     BLOB NOT NULL
);

-- ── Run settings: how an anchor is run, not what it judges. **Mutable**:
-- no append-only trigger below, because changing one settles nothing and
-- so owes no sealed rationale.

CREATE TABLE IF NOT EXISTS settings (
    anchor        TEXT    PRIMARY KEY,
    retain        TEXT    NOT NULL,   -- Retain, snake_case
    cadence_secs  INTEGER,            -- NULL defers to the deployment default
    budget_ms     INTEGER,            -- NULL defers to the deployment default
    facts         TEXT    NOT NULL    -- Recorded, snake_case. Beside retain, not
                  DEFAULT 'plain'       -- inside it: retain decides whether an
                                        -- unchanged observation is written at all,
                                        -- this decides whether one may be plaintext
);

-- ── Sightings: how often we looked and found the anchor where it should be,
-- and when we last did. **Mutable**, like settings and the queue: a look that
-- found nothing new settles nothing, so it owes no sealed rationale and has no
-- business in an append-only log.

CREATE TABLE IF NOT EXISTS sighting (
    anchor   TEXT PRIMARY KEY,
    count    INTEGER NOT NULL DEFAULT 0,
    last_at  TEXT                        -- RFC3339, as the entries spell it
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
CREATE TRIGGER IF NOT EXISTS binding_revocations_no_update BEFORE UPDATE ON binding_revocations
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS binding_revocations_no_delete BEFORE DELETE ON binding_revocations
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS binding_revoked_tags_no_update BEFORE UPDATE ON binding_revoked_tags
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS binding_revoked_tags_no_delete BEFORE DELETE ON binding_revoked_tags
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

pub const V6_TO_V7: &str = r#"
ALTER TABLE settings ADD COLUMN budget_ms INTEGER;
"#;

pub const V7_TO_V8: &str = r#"
CREATE TABLE IF NOT EXISTS sighting (
    anchor   TEXT PRIMARY KEY,
    count    INTEGER NOT NULL DEFAULT 0,
    last_at  TEXT
);

INSERT INTO sighting (anchor, count, last_at)
SELECT j.anchor, COUNT(*), (
    SELECT json_extract(prior.body, '$.at') FROM journal prior
    WHERE prior.anchor = j.anchor
      AND json_extract(prior.body, '$.entry') IN ('open', 'transition', 'still')
    ORDER BY prior.seq DESC LIMIT 1
)
FROM journal j
WHERE json_extract(j.body, '$.entry') IN ('open', 'transition', 'still')
GROUP BY j.anchor
ON CONFLICT(anchor) DO NOTHING;
"#;

pub const V8_TO_V9: &str = r#"
DROP TRIGGER IF EXISTS bindings_no_update;
DROP TRIGGER IF EXISTS bindings_no_delete;
DROP TRIGGER IF EXISTS binding_anchors_no_update;
DROP TRIGGER IF EXISTS binding_anchors_no_delete;

CREATE TABLE bindings_v9 (
    seq             INTEGER PRIMARY KEY AUTOINCREMENT,
    reference       TEXT NOT NULL,
    body            TEXT NOT NULL,
    bound_version   TEXT,
    bound_at_seq    INTEGER,
    source          TEXT NOT NULL,
    asserted_at     TEXT,
    baseline_at_seq INTEGER
);

-- Every row that predates this column was asserted either by a note declaring
-- a coordinate or by a person typing `gmr bind`, and nothing recorded which.
-- `unknown` says that. Calling them self-attested would be a fact this store
-- does not have, and self-attested is the one word the provenance question
-- reads as "no independent evidence".
INSERT INTO bindings_v9 (seq, reference, body, bound_version, bound_at_seq, source, asserted_at, baseline_at_seq)
SELECT seq, reference, body, bound_version, bound_at_seq, 'unknown', NULL, seq FROM bindings;

CREATE TABLE binding_anchors_v9 (
    seq        INTEGER NOT NULL REFERENCES bindings_v9(seq),
    anchor     TEXT    NOT NULL,
    PRIMARY KEY (seq, anchor)
);
INSERT INTO binding_anchors_v9 (seq, anchor) SELECT seq, anchor FROM binding_anchors;

-- The child goes first: with foreign keys on, dropping the parent while a
-- child still references it is a constraint violation, and the pragma that
-- would silence it cannot be moved inside this transaction.
DROP TABLE binding_anchors;
DROP TABLE bindings;
ALTER TABLE bindings_v9 RENAME TO bindings;
ALTER TABLE binding_anchors_v9 RENAME TO binding_anchors;

CREATE INDEX IF NOT EXISTS bindings_by_reference ON bindings(reference, seq);
CREATE INDEX IF NOT EXISTS binding_anchors_by_anchor ON binding_anchors(anchor);

CREATE TRIGGER bindings_no_update BEFORE UPDATE ON bindings
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER bindings_no_delete BEFORE DELETE ON bindings
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER binding_anchors_no_update BEFORE UPDATE ON binding_anchors
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER binding_anchors_no_delete BEFORE DELETE ON binding_anchors
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;

CREATE TABLE IF NOT EXISTS binding_revocations (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    reference   TEXT NOT NULL,
    anchor      TEXT NOT NULL,
    source      TEXT NOT NULL,
    revoked_at  TEXT
);
CREATE INDEX IF NOT EXISTS binding_revocations_by_anchor ON binding_revocations(anchor);

CREATE TABLE IF NOT EXISTS binding_revoked_tags (
    revocation  INTEGER NOT NULL REFERENCES binding_revocations(seq),
    binding     INTEGER NOT NULL,
    anchor      TEXT    NOT NULL,
    PRIMARY KEY (revocation, binding, anchor)
);

CREATE TRIGGER binding_revocations_no_update BEFORE UPDATE ON binding_revocations
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER binding_revocations_no_delete BEFORE DELETE ON binding_revocations
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER binding_revoked_tags_no_update BEFORE UPDATE ON binding_revoked_tags
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER binding_revoked_tags_no_delete BEFORE DELETE ON binding_revoked_tags
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
"#;

pub const V9_TO_V10: &str = r#"
ALTER TABLE settings ADD COLUMN facts TEXT NOT NULL DEFAULT 'plain';
"#;

pub const V10_TO_V11_OPEN: &str = r#"
DROP TRIGGER IF EXISTS journal_no_update;
DROP TRIGGER IF EXISTS journal_no_delete;
ALTER TABLE journal ADD COLUMN prev TEXT;
ALTER TABLE journal ADD COLUMN hash TEXT;
"#;

pub const V10_TO_V11_CLOSE: &str = r#"
CREATE TRIGGER IF NOT EXISTS journal_no_update BEFORE UPDATE ON journal
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
CREATE TRIGGER IF NOT EXISTS journal_no_delete BEFORE DELETE ON journal
    BEGIN SELECT RAISE(ABORT, 'append_only'); END;
"#;

pub const V11_TO_V12: &str = r#"
ALTER TABLE bindings ADD COLUMN saw TEXT;
"#;

-- Migration: 002_retention_helpers
-- Description: Add views and materialized state for compact_old_jsonl and vacuum_aggregates operations

-- Materialized aggregate: session summary for quick retention queries
CREATE TABLE IF NOT EXISTS session_aggregates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL UNIQUE,
    character TEXT NOT NULL,
    server TEXT NOT NULL,
    start_ts INTEGER NOT NULL,
    end_ts INTEGER,
    duration_secs INTEGER,
    total_combat_events INTEGER DEFAULT 0,
    total_deaths INTEGER DEFAULT 0,
    total_pulls INTEGER DEFAULT 0,
    total_loot_pp REAL DEFAULT 0.0,
    last_updated_at INTEGER DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);

-- Index for retention queries (older_than_days)
CREATE INDEX IF NOT EXISTS idx_aggregates_start_ts ON session_aggregates(start_ts);

-- View: sessions eligible for compact_old_jsonl (older than N days)
CREATE VIEW IF NOT EXISTS sessions_eligible_for_compact AS
SELECT
    sa.session_id,
    sa.character,
    sa.server,
    sa.start_ts,
    sa.duration_secs,
    (strftime('%s', 'now') - sa.start_ts) as age_seconds
FROM session_aggregates sa
WHERE (strftime('%s', 'now') - sa.start_ts) > (7 * 86400)  -- 7 days default
ORDER BY sa.start_ts ASC;

-- View: sessions eligible for vacuum_aggregates (old retention policy)
CREATE VIEW IF NOT EXISTS sessions_eligible_for_vacuum AS
SELECT
    sa.session_id,
    sa.character,
    sa.server,
    sa.start_ts,
    (strftime('%s', 'now') - sa.start_ts) as age_seconds
FROM session_aggregates sa
WHERE (strftime('%s', 'now') - sa.start_ts) > (30 * 86400)  -- 30 days default
ORDER BY sa.start_ts ASC;

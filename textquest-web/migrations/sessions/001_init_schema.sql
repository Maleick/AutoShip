-- Migration: 001_init_schema
-- Description: Initialize core sessions and decision tracking schema for self-improvement loop

-- Schema version tracking table
CREATE TABLE IF NOT EXISTS schema_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version INTEGER NOT NULL UNIQUE,
    name TEXT NOT NULL,
    applied_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Core session metadata
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character TEXT NOT NULL,
    server TEXT NOT NULL,
    start_ts INTEGER NOT NULL,
    end_ts INTEGER,
    zone_seq TEXT,
    build_sha TEXT,
    duration_secs INTEGER,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Combat events with full telemetry
CREATE TABLE IF NOT EXISTS combat_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    mob_name TEXT,
    mob_id INTEGER,
    ability TEXT,
    damage INTEGER,
    hp_pct_before REAL,
    hp_pct_after REAL,
    mana_pct REAL,
    target_distance REAL,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);

-- Death events for failure analysis
CREATE TABLE IF NOT EXISTS deaths (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    zone TEXT,
    x REAL,
    y REAL,
    z REAL,
    killer TEXT,
    last_action TEXT,
    recovery_seconds INTEGER,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);

-- Stuck/waypoint events
CREATE TABLE IF NOT EXISTS stuck_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    x REAL,
    y REAL,
    z REAL,
    route_id TEXT,
    recovered_via TEXT,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);

-- Mob pull events
CREATE TABLE IF NOT EXISTS pulls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    mob_id INTEGER,
    mob_name TEXT,
    route_node TEXT,
    time_to_engage_ms INTEGER,
    success INTEGER,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);

-- Loot acquisition
CREATE TABLE IF NOT EXISTS loot (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    mob_id INTEGER,
    item_id INTEGER,
    item_name TEXT,
    value_pp_estimate REAL,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);

-- Per-node route costs and safety metrics
CREATE TABLE IF NOT EXISTS route_costs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    route_id TEXT,
    node_idx INTEGER,
    traversal_ms INTEGER,
    deaths_at_node INTEGER,
    stuck_at_node INTEGER,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);

-- Rotation ticks for ability execution timing
CREATE TABLE IF NOT EXISTS rotation_ticks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    ability TEXT,
    attempted INTEGER,
    succeeded INTEGER,
    gcd_blocked INTEGER,
    oom INTEGER,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);

-- Indices for query performance
CREATE INDEX IF NOT EXISTS idx_sessions_character ON sessions(character, start_ts);
CREATE INDEX IF NOT EXISTS idx_sessions_server ON sessions(server);
CREATE INDEX IF NOT EXISTS idx_combat_session_ts ON combat_events(session_id, ts);
CREATE INDEX IF NOT EXISTS idx_deaths_session ON deaths(session_id);
CREATE INDEX IF NOT EXISTS idx_pulls_session ON pulls(session_id);
CREATE INDEX IF NOT EXISTS idx_pulls_success ON pulls(session_id, success);
CREATE INDEX IF NOT EXISTS idx_loot_session ON loot(session_id);
CREATE INDEX IF NOT EXISTS idx_route_costs_session ON route_costs(session_id, route_id);
CREATE INDEX IF NOT EXISTS idx_rotation_ticks_session ON rotation_ticks(session_id, ts);
CREATE INDEX IF NOT EXISTS idx_stuck_events_session ON stuck_events(session_id);

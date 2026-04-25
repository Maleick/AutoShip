//! SQLite schema for aggregate metrics tables.

pub struct AggregateSchema;

impl AggregateSchema {
    /// SQL schema for all aggregate tables.
    /// Tables are idempotent via session_id PK.
    pub const SCHEMA: &'static str = r#"
-- Session-level aggregates (primary key = session_id for idempotency)
CREATE TABLE IF NOT EXISTS session_aggregates (
    session_id          TEXT PRIMARY KEY,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
    character_names     TEXT,
    zone_counts         TEXT,
    total_events        INTEGER,
    duration_seconds    INTEGER
);

-- Kill-time-per-mob: mean, p50, p90 by (zone, party-comp)
CREATE TABLE IF NOT EXISTS kill_time_per_mob (
    session_id          TEXT NOT NULL,
    zone                TEXT NOT NULL,
    party_comp          TEXT NOT NULL,
    mean_ms             REAL,
    p50_ms              REAL,
    p90_ms              REAL,
    kill_count          INTEGER DEFAULT 0,
    PRIMARY KEY (session_id, zone, party_comp),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Net plat/hour by zone: (loot_value - deaths_cost - consumable_cost) / hours
CREATE TABLE IF NOT EXISTS net_plat_per_hour_zone (
    session_id          TEXT NOT NULL,
    zone                TEXT NOT NULL,
    gross_plat          INTEGER DEFAULT 0,
    deaths_cost         INTEGER DEFAULT 0,
    consumable_cost     INTEGER DEFAULT 0,
    net_plat            INTEGER DEFAULT 0,
    duration_seconds    INTEGER DEFAULT 0,
    plat_per_hour       REAL,
    PRIMARY KEY (session_id, zone),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Time-to-recover-after-death: median, p90 in seconds
CREATE TABLE IF NOT EXISTS death_recovery_time (
    session_id          TEXT NOT NULL,
    death_count         INTEGER DEFAULT 0,
    median_seconds      REAL,
    p90_seconds         REAL,
    PRIMARY KEY (session_id),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Spell efficiency: successful_casts / total_attempts per ability
CREATE TABLE IF NOT EXISTS spell_efficiency (
    session_id          TEXT NOT NULL,
    spell_name          TEXT NOT NULL,
    total_attempts      INTEGER DEFAULT 0,
    successful_casts    INTEGER DEFAULT 0,
    success_rate        REAL,
    PRIMARY KEY (session_id, spell_name),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Pull-cadence distribution: time between pulls vs recovery patterns
CREATE TABLE IF NOT EXISTS pull_cadence (
    session_id          TEXT NOT NULL,
    mean_pull_interval_s   REAL,
    p50_pull_interval_s    REAL,
    p90_pull_interval_s    REAL,
    mean_mana_recovery_s   REAL,
    mean_hp_recovery_s     REAL,
    pull_count             INTEGER DEFAULT 0,
    PRIMARY KEY (session_id),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Ability contribution rank: (damage × times_used) / mana_spent per ability
CREATE TABLE IF NOT EXISTS ability_contribution (
    session_id          TEXT NOT NULL,
    ability_name        TEXT NOT NULL,
    damage_total        INTEGER DEFAULT 0,
    times_used          INTEGER DEFAULT 0,
    mana_spent          INTEGER DEFAULT 0,
    contribution_rank   REAL,
    PRIMARY KEY (session_id, ability_name),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Route-node death rate: deaths_at_node / traversals by zone location
CREATE TABLE IF NOT EXISTS route_node_death_rate (
    session_id          TEXT NOT NULL,
    zone                TEXT NOT NULL,
    node_x              REAL NOT NULL,
    node_y              REAL NOT NULL,
    node_z              REAL NOT NULL,
    death_count         INTEGER DEFAULT 0,
    traversal_count     INTEGER DEFAULT 0,
    death_rate          REAL,
    PRIMARY KEY (session_id, zone, node_x, node_y, node_z),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Stuck-frequency heatmap: (zone, x_bucket, y_bucket) → stuck_count
CREATE TABLE IF NOT EXISTS stuck_heatmap (
    session_id          TEXT NOT NULL,
    zone                TEXT NOT NULL,
    x_bucket            INTEGER NOT NULL,
    y_bucket            INTEGER NOT NULL,
    stuck_count         INTEGER DEFAULT 0,
    PRIMARY KEY (session_id, zone, x_bucket, y_bucket),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- XP Tracking: Track XP gain and AA gain rates per session per character
CREATE TABLE IF NOT EXISTS xp_tracking_session (
    session_id          TEXT NOT NULL,
    character_name      TEXT NOT NULL,
    start_level         INTEGER,
    end_level           INTEGER,
    xp_percent_start    REAL DEFAULT 0,
    xp_percent_end      REAL DEFAULT 0,
    xp_gained_percent   REAL DEFAULT 0,
    aa_count_start      INTEGER DEFAULT 0,
    aa_count_end        INTEGER DEFAULT 0,
    aa_gained           INTEGER DEFAULT 0,
    duration_seconds    INTEGER DEFAULT 0,
    xp_per_hour         REAL,
    aa_per_hour         REAL,
    PRIMARY KEY (session_id, character_name),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Kill Tracking: Track kills per character per session, grouped by mob/zone
CREATE TABLE IF NOT EXISTS kill_tracking_session (
    session_id          TEXT NOT NULL,
    character_name      TEXT NOT NULL,
    mob_name            TEXT NOT NULL,
    zone                TEXT NOT NULL,
    kill_count          INTEGER DEFAULT 0,
    is_named            BOOLEAN DEFAULT 0,
    duration_seconds    INTEGER DEFAULT 0,
    kills_per_hour      REAL,
    PRIMARY KEY (session_id, character_name, mob_name, zone),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Plat Tracking: Per-transaction logging of platinum changes
CREATE TABLE IF NOT EXISTS plat_transaction_log (
    session_id          TEXT NOT NULL,
    character_name      TEXT NOT NULL,
    timestamp           INTEGER NOT NULL,
    transaction_type    TEXT NOT NULL,
    amount_platinum     INTEGER DEFAULT 0,
    amount_gold         INTEGER DEFAULT 0,
    amount_silver       INTEGER DEFAULT 0,
    amount_copper       INTEGER DEFAULT 0,
    description         TEXT,
    PRIMARY KEY (session_id, character_name, timestamp),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Plat Summary: Session-level platinum totals and rates
CREATE TABLE IF NOT EXISTS plat_summary_session (
    session_id          TEXT NOT NULL,
    character_name      TEXT NOT NULL,
    gross_platinum      INTEGER DEFAULT 0,
    net_platinum        INTEGER DEFAULT 0,
    duration_seconds    INTEGER DEFAULT 0,
    platinum_per_hour   REAL,
    transaction_count   INTEGER DEFAULT 0,
    PRIMARY KEY (session_id, character_name),
    FOREIGN KEY(session_id) REFERENCES session_aggregates(session_id)
);

-- Track which JSONL files have been aggregated (for --all-pending)
CREATE TABLE IF NOT EXISTS aggregated_sessions (
    session_id          TEXT PRIMARY KEY,
    aggregated_at       TEXT NOT NULL DEFAULT (datetime('now')),
    jsonl_path          TEXT
);

CREATE INDEX IF NOT EXISTS idx_aggregated_sessions_at ON aggregated_sessions(aggregated_at);
CREATE INDEX IF NOT EXISTS idx_kill_time_zone ON kill_time_per_mob(zone);
CREATE INDEX IF NOT EXISTS idx_net_plat_zone ON net_plat_per_hour_zone(zone);
CREATE INDEX IF NOT EXISTS idx_spell_eff_spell ON spell_efficiency(spell_name);
CREATE INDEX IF NOT EXISTS idx_ability_contrib_ability ON ability_contribution(ability_name);
CREATE INDEX IF NOT EXISTS idx_route_node_zone ON route_node_death_rate(zone);
CREATE INDEX IF NOT EXISTS idx_stuck_heatmap_zone ON stuck_heatmap(zone);
CREATE INDEX IF NOT EXISTS idx_xp_tracking_char ON xp_tracking_session(character_name);
CREATE INDEX IF NOT EXISTS idx_kill_tracking_mob ON kill_tracking_session(mob_name);
CREATE INDEX IF NOT EXISTS idx_kill_tracking_zone ON kill_tracking_session(zone);
CREATE INDEX IF NOT EXISTS idx_plat_trans_char ON plat_transaction_log(character_name);
CREATE INDEX IF NOT EXISTS idx_plat_trans_type ON plat_transaction_log(transaction_type);
CREATE INDEX IF NOT EXISTS idx_plat_summary_char ON plat_summary_session(character_name);
"#;
}

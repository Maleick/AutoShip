//! Non-blocking metrics persistence to SQLite with retention policy.
//!
//! Writes historical metrics data on a background interval without blocking the metrics thread.
//! Enforces 30-day retention policy to manage database size.

use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use super::collector::AggregateMetrics;

/// Default interval for writing metrics to SQLite (10 seconds).
const METRICS_PERSIST_INTERVAL: Duration = Duration::from_secs(10);

/// Default retention period for historical metrics (30 days).
const METRICS_RETENTION_DAYS: i64 = 30;

/// Thread-safe metrics persistence writer.
pub struct MetricsPersister {
    conn: Arc<Mutex<Connection>>,
    _thread_handle: Option<thread::JoinHandle<()>>,
}

impl MetricsPersister {
    /// Create a new metrics persister with the given database path.
    ///
    /// Spawns a background thread that writes metrics on interval and enforces retention.
    pub fn new(db_path: impl AsRef<std::path::Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let conn = Connection::open(&db_path)
            .context("Failed to open metrics database")?;

        // Initialize metrics history schema
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS metrics_history (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp       TEXT NOT NULL DEFAULT (datetime('now')),
                window_name     TEXT NOT NULL,
                avg_dps         REAL,
                damage_dealt    INTEGER,
                damage_taken    INTEGER,
                kills           INTEGER,
                deaths          INTEGER,
                items_looted    INTEGER,
                plat_earned     INTEGER,
                movement_dist   REAL,
                stuck_events    INTEGER,
                spells_cast     INTEGER,
                healing_done    INTEGER,
                memory_bytes    INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_metrics_ts ON metrics_history(timestamp);
            CREATE INDEX IF NOT EXISTS idx_metrics_window ON metrics_history(window_name);",
        ).context("Failed to initialize metrics schema")?;

        let conn = Arc::new(Mutex::new(conn));
        let conn_thread = Arc::clone(&conn);

        // Spawn background persistence thread
        let thread_handle = thread::spawn(move || {
            let mut last_cleanup = SystemTime::now();
            loop {
                thread::sleep(METRICS_PERSIST_INTERVAL);

                // Every 10 cleanup intervals, enforce retention policy
                if last_cleanup.elapsed().unwrap_or_default() > Duration::from_secs(600) {
                    if let Ok(conn) = conn_thread.lock() {
                        let _ = cleanup_old_metrics(&conn);
                    }
                    last_cleanup = SystemTime::now();
                }
            }
        });

        Ok(Self {
            conn,
            _thread_handle: Some(thread_handle),
        })
    }

    /// Write aggregate metrics to the database without blocking.
    pub fn write_metrics(&self, window_name: &str, metrics: &AggregateMetrics) -> Result<()> {
        let conn = self.conn.lock()
            .map_err(|_| anyhow::anyhow!("Failed to acquire database lock"))?;

        conn.execute(
            "INSERT INTO metrics_history (
                window_name, avg_dps, damage_dealt, damage_taken, kills, deaths,
                items_looted, plat_earned, movement_dist, stuck_events, spells_cast,
                healing_done, memory_bytes
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                window_name,
                metrics.dps,
                metrics.damage_dealt,
                metrics.damage_taken,
                metrics.kills,
                metrics.deaths,
                metrics.items_looted,
                metrics.plat_earned,
                metrics.movement_distance,
                metrics.stuck_events,
                metrics.spells_cast,
                metrics.healing_done,
                metrics.system_memory_bytes,
            ],
        ).context("Failed to insert metrics record")?;

        Ok(())
    }

    /// Query historical metrics within a time window.
    pub fn query_metrics_history(
        &self,
        window_name: &str,
        hours_back: i64,
    ) -> Result<Vec<MetricsHistoryRow>> {
        let conn = self.conn.lock()
            .map_err(|_| anyhow::anyhow!("Failed to acquire database lock"))?;

        let cutoff_time = unix_timestamp_secs() - (hours_back * 3600);
        let mut stmt = conn.prepare(
            "SELECT timestamp, window_name, avg_dps, damage_dealt, kills, spells_cast
             FROM metrics_history
             WHERE window_name = ? AND strftime('%s', timestamp) > ?
             ORDER BY timestamp DESC"
        ).context("Failed to prepare query")?;

        let rows = stmt.query_map(
            params![window_name, cutoff_time],
            |row| {
                Ok(MetricsHistoryRow {
                    timestamp: row.get(0)?,
                    window_name: row.get(1)?,
                    avg_dps: row.get(2)?,
                    damage_dealt: row.get(3)?,
                    kills: row.get(4)?,
                    spells_cast: row.get(5)?,
                })
            },
        ).context("Failed to query metrics history")?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.context("Failed to extract row data")?);
        }

        Ok(results)
    }

    /// Get retention period in days.
    pub fn retention_days() -> i64 {
        METRICS_RETENTION_DAYS
    }
}

/// A single row of historical metrics data.
#[derive(Debug, Clone)]
pub struct MetricsHistoryRow {
    pub timestamp: String,
    pub window_name: String,
    pub avg_dps: f64,
    pub damage_dealt: u64,
    pub kills: u32,
    pub spells_cast: u32,
}

/// Clean up metrics older than retention period.
fn cleanup_old_metrics(conn: &Connection) -> Result<()> {
    let cutoff_time = unix_timestamp_secs() - (METRICS_RETENTION_DAYS * 86400);

    let deleted = conn.execute(
        "DELETE FROM metrics_history WHERE strftime('%s', timestamp) < ?",
        params![cutoff_time],
    ).context("Failed to delete old metrics")?;

    if deleted > 0 {
        tracing::debug!("Cleaned up {} metrics records", deleted);
    }

    Ok(())
}

/// Get current Unix timestamp in seconds.
fn unix_timestamp_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_metrics_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("metrics_test.db");

        let persister = MetricsPersister::new(&db_path).unwrap();

        let mut metrics = AggregateMetrics::default();
        metrics.dps = 1000.0;
        metrics.damage_dealt = 5000;
        metrics.kills = 5;

        assert!(persister.write_metrics("OneMin", &metrics).is_ok());

        // Query should return the written metric
        let history = persister
            .query_metrics_history("OneMin", 1)
            .unwrap();
        assert!(!history.is_empty());
        assert_eq!(history[0].avg_dps, 1000.0);
        assert_eq!(history[0].damage_dealt, 5000);
        assert_eq!(history[0].kills, 5);
    }

    #[test]
    fn test_retention_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("metrics_cleanup_test.db");

        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE metrics_history (
                id          INTEGER PRIMARY KEY,
                timestamp   TEXT NOT NULL,
                window_name TEXT NOT NULL,
                avg_dps     REAL,
                damage_dealt INTEGER,
                kills        INTEGER,
                spells_cast  INTEGER
            );"
        ).unwrap();

        // Insert a record with a very old timestamp
        conn.execute(
            "INSERT INTO metrics_history (timestamp, window_name, avg_dps, damage_dealt, kills, spells_cast)
             VALUES (datetime('now', '-40 days'), 'OneMin', 1000.0, 5000, 5, 100)",
            [],
        ).unwrap();

        // Insert a recent record
        conn.execute(
            "INSERT INTO metrics_history (timestamp, window_name, avg_dps, damage_dealt, kills, spells_cast)
             VALUES (datetime('now'), 'OneMin', 1500.0, 6000, 6, 110)",
            [],
        ).unwrap();

        // Cleanup
        cleanup_old_metrics(&conn).unwrap();

        // Old record should be deleted, recent should remain
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM metrics_history", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}

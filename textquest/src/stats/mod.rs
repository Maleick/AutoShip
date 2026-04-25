//! Session aggregation — materialize JSONL events into SQLite aggregate tables.
//!
//! On demand (`textquest stats compact`) or on-session-close hook:
//! reads raw event JSONL, computes derived metrics, stores in aggregate tables.

pub mod aggregator;
pub mod schema;

pub use aggregator::{SessionAggregator, SessionEvent};
pub use schema::AggregateSchema;

use anyhow::Result;
use std::path::Path;

/// Compact a single session's JSONL events into aggregate tables (idempotent).
pub fn compact_session(metrics_db_path: &Path, session_dir: &Path, session_id: &str) -> Result<()> {
    let aggregator = SessionAggregator::new(metrics_db_path)?;
    aggregator.compact_session(session_dir, session_id)
}

/// Compact all pending (non-aggregated) sessions.
pub fn compact_all_pending(metrics_db_path: &Path, event_base_dir: &Path) -> Result<()> {
    let aggregator = SessionAggregator::new(metrics_db_path)?;
    aggregator.compact_all_pending(event_base_dir)
}

//! Sampling scheduler — drives periodic metric collection and flush.
//!
//! # Design intent (Phase 4.2a — see docs/design/performance-monitoring/design.md)
//!
//! The [`SamplingScheduler`] runs as a background tokio task and orchestrates
//! the three-tier cadence:
//!
//! | Cadence | Action |
//! |---------|--------|
//! | 1 second | Snapshot operational gauges (memory, IPC latency) into [`AdminMonitoringStore`] |
//! | 60 seconds | Flush accumulated counters to SQLite via [`MetricsStore`]; update Prometheus state |
//! | 5 minutes | Compute [`BaselineScorecard`] delta for optimization feedback |
//! | Session end | Final rollup + archival write |
//!
//! ## Circuit breaker integration (#1138)
//!
//! Before each sampling tick the scheduler must consult the circuit breaker state:
//! - `Closed` → full sampling
//! - `Open` → admin metrics only; combat/economy sampling suspended
//! - `HalfOpen` → reduced cadence (10s gauges, no SQLite flush until re-closed)
//!
//! ## Performance budget
//!
//! - Per-event update: < 1 µs (lock-free atomics only on hot path)
//! - 1-second gauge snapshot: < 500 µs total
//! - 60-second SQLite flush: < 10 ms
//!
//! ## Not yet implemented
//!
//! This module is a design scaffold. Implementation tracked in #1112.

/// Drives periodic metric collection on a configurable cadence.
///
/// Create via [`SamplingScheduler::new`] and spawn with
/// [`SamplingScheduler::run`]. The scheduler holds weak references to the
/// shared metric stores so it shuts down cleanly when the owning context drops.
///
/// # Example (not yet wired)
///
/// ```ignore
/// let scheduler = SamplingScheduler::new(config, collector.clone(), store.clone());
/// tokio::spawn(scheduler.run(shutdown_rx));
/// ```
pub struct SamplingScheduler {
    // TODO (#1112): Add fields:
    //   config: SamplingConfig,
    //   collector: Arc<MetricsCollector>,
    //   store: Arc<MetricsStore>,
    //   admin_store: Arc<AdminMonitoringStore>,
    //   prometheus: Option<Arc<PrometheusExporter>>,
    //   alert_tx: mpsc::Sender<AlertEvent>,
}

impl SamplingScheduler {
    /// Construct a new scheduler. No background task is started until
    /// [`SamplingScheduler::run`] is called.
    pub fn new() -> Self {
        // TODO (#1112): accept config + store handles
        Self {}
    }

    /// Spawn the scheduler loop. Returns when `shutdown_rx` fires or all store
    /// handles are dropped.
    ///
    /// Implementation placeholder — no-op until #1112.
    pub async fn run(self) {
        // TODO (#1112): implement three-tier sampling loop
    }
}

impl Default for SamplingScheduler {
    fn default() -> Self {
        Self::new()
    }
}

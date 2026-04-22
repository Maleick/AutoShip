//! Prometheus text-format exporter — optional HTTP `/metrics` endpoint.
//!
//! # Design intent (Phase 4.2a — see docs/design/performance-monitoring/design.md)
//!
//! The [`PrometheusExporter`] serves Prometheus-compatible text output on
//! `localhost:9100/metrics` (port and path configurable). It is **disabled by
//! default** and only activated when `metrics.prometheus_enabled = true` in
//! `config.toml`.
//!
//! ## Metric naming convention
//!
//! All exported metrics use the `tq_` prefix. Labels follow Prometheus
//! conventions (`client_id`, `zone`, `error_kind`, etc.).
//!
//! ## Scrape cadence
//!
//! Recommend a 60-second Prometheus scrape interval — this matches the
//! [`SamplingScheduler`] flush cadence so counters advance uniformly between
//! scrapes.
//!
//! ## Performance budget
//!
//! Scrape response latency: < 5 ms (target), 20 ms (hard limit). The exporter
//! reads pre-computed state written by the [`SamplingScheduler`]; it never
//! touches SQLite or the IPC path during a scrape.
//!
//! ## Not yet implemented
//!
//! This module is a design scaffold. Implementation tracked in #1112.

/// Serves Prometheus text-format metrics over HTTP.
///
/// Activated via config; no-op when disabled.
///
/// ```ignore
/// // In config.toml:
/// // [metrics]
/// // prometheus_enabled = true
/// // prometheus_port = 9100
/// // prometheus_path = "/metrics"
/// ```
pub struct PrometheusExporter {
    // TODO (#1112): Add fields:
    //   port: u16,
    //   path: String,
    //   state: Arc<PrometheusState>,  // written by SamplingScheduler, read by scrape handler
}

impl PrometheusExporter {
    /// Build a disabled no-op exporter. Call [`PrometheusExporter::enabled`]
    /// to construct a real one.
    pub fn disabled() -> Self {
        // TODO (#1112): implement
        Self {}
    }

    /// Start the HTTP listener. Returns immediately if the exporter is disabled.
    ///
    /// Implementation placeholder — no-op until #1112.
    pub async fn serve(self) {
        // TODO (#1112): bind Axum/hyper listener, serve /metrics handler
    }
}

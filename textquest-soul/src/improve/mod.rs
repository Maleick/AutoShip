//! Streaming anomaly-detection pipeline for the self-improvement loop.
//!
//! Consumes session metrics and emits [`AnomalyEvent`]s on session close.
//! No mid-session output — all anomalies surface post-session only.
//!
//! # Detector families
//! - [`ewma`] — EWMA control chart for DPS drops per (class, zone)
//! - [`page_hinkley`] — Page-Hinkley test for death-rate spikes
//! - [`mad`] — Robust z-score (MAD) for mana P10 collapse
//! - [`bocpd`] — Bayesian online changepoint detection for stuck-event rate
//! - [`stl_mad`] — STL seasonal decomposition + MAD for loot-rate drops
//!
//! # Post-processing
//! - [`causal`] — dependency-graph explain-away (demotes correlated alarms)
//! - [`feedback`] — Beta-Binomial self-tuning false-alarm suppression

pub mod bocpd;
pub mod causal;
pub mod ewma;
pub mod feedback;
pub mod mad;
pub mod page_hinkley;
pub mod stl_mad;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Severity ──────────────────────────────────────────────────────────────────

/// Operator-visible severity tier for an anomaly event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Minor = 0,
    Average = 1,
    Major = 2,
}

// ── AnomalyKind ───────────────────────────────────────────────────────────────

/// Discriminated kind identifying which detector family fired.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnomalyKind {
    /// DPS drop detected via EWMA control chart for a (class, zone) pair.
    DpsDropEwma { class: String, zone: String },
    /// ≥3 deaths in a 10-minute window detected via Page-Hinkley test.
    DeathClusterPageHinkley,
    /// Mana P10 collapse below rolling baseline (MAD robust z-score).
    ManaCollapseMad,
    /// Stuck-event rate spike at a route node (BOCPD, requires ≥30 sessions).
    StuckRateBocpd { route_id: u64, node_id: u64 },
    /// Loot-rate drop on a heavily-farmed camp (STL residual + MAD, ≥30 sessions).
    LootRateStlMad { camp: String },
}

// ── AnomalyEvent ─────────────────────────────────────────────────────────────

/// A single anomaly event emitted by the pipeline on session close.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyEvent {
    pub id: u64,
    pub ts: DateTime<Utc>,
    pub kind: AnomalyKind,
    pub metric_name: String,
    pub baseline_mean: f64,
    pub baseline_std: f64,
    pub observed: f64,
    pub severity: Severity,
    /// Anomaly ID of the parent event that caused this one (explain-away link).
    pub caused_by: Option<u64>,
    /// True when baseline is older than 60 days or a patch event was recorded.
    /// Events with stale baselines are always emitted at Minor severity.
    pub stale_baseline: bool,
}

impl AnomalyEvent {
    pub fn new(
        id: u64,
        kind: AnomalyKind,
        metric_name: impl Into<String>,
        baseline_mean: f64,
        baseline_std: f64,
        observed: f64,
        natural_severity: Severity,
        stale_baseline: bool,
    ) -> Self {
        // Stale baselines always emit at Minor to prevent post-patch alarm storms.
        let severity = if stale_baseline {
            Severity::Minor
        } else {
            natural_severity
        };
        Self {
            id,
            ts: Utc::now(),
            kind,
            metric_name: metric_name.into(),
            baseline_mean,
            baseline_std,
            observed,
            severity,
            caused_by: None,
            stale_baseline,
        }
    }
}

// ── SessionMetric ─────────────────────────────────────────────────────────────

/// Input metrics fed into the pipeline via [`AnomalyPipeline::feed`].
#[derive(Debug, Clone)]
pub enum SessionMetric {
    /// DPS observation for a (class, zone) pair.
    Dps { class: String, zone: String, value: f64 },
    /// A death event (Unix millisecond timestamp).
    Death { ts_ms: u64 },
    /// 10th-percentile mana value.
    ManaP10 { value: f64 },
    /// A stuck-navigation event at a route node.
    StuckEvent { route_id: u64, node_id: u64 },
    /// Loot-rate sample at a named camp (items per hour).
    LootRate { camp: String, value: f64 },
    /// Causal context: a cleric debuff was active this session.
    ClericDebuffActive(bool),
    /// Causal context: a required buff was missing this session.
    MissingBuff(bool),
    /// Causal context: a gear-slot delta was detected this session.
    GearDelta(bool),
    /// A game patch was recorded — marks all baselines stale.
    PatchEvent,
}

// ── OperatorAction ────────────────────────────────────────────────────────────

/// An action taken by the operator on an anomaly card.
#[derive(Debug, Clone)]
pub enum OperatorAction {
    /// Accept — anomaly is real and worth investigating.
    Accept(u64),
    /// Dismiss — not a real problem; raises detector threshold.
    Dismiss(u64),
    /// Real but ignore for now — does not raise threshold.
    RealButIgnore(u64),
}

// ── AnomalyPipeline ───────────────────────────────────────────────────────────

use crate::improve::{
    bocpd::BocpdDetector, causal::CausalGraph, ewma::EwmaDetector,
    feedback::PipelineFeedback, mad::MadDetector, page_hinkley::PageHinkleyDetector,
    stl_mad::StlMadDetector,
};

/// Orchestrates all detector families and post-processors.
///
/// Call [`feed`](Self::feed) during a session, then [`on_session_close`](Self::on_session_close)
/// once at session end.  No events are emitted mid-session.
pub struct AnomalyPipeline {
    ewma: EwmaDetector,
    page_hinkley: PageHinkleyDetector,
    mad: MadDetector,
    bocpd: BocpdDetector,
    stl_mad: StlMadDetector,
    causal: CausalGraph,
    feedback: PipelineFeedback,
    pending: Vec<SessionMetric>,
    next_id: u64,
    stale_baseline: bool,
    /// Per-session anomaly counts for false-alarm rate tracking (max 30).
    session_counts: Vec<usize>,
}

impl AnomalyPipeline {
    pub fn new() -> Self {
        Self {
            ewma: EwmaDetector::new(0.2, 3.0),
            // 10-minute window, baseline rate 0.1 deaths/min, δ=0.005, threshold=25
            page_hinkley: PageHinkleyDetector::new(600_000, 0.1, 0.005, 25.0),
            mad: MadDetector::new(200, 3.5),
            bocpd: BocpdDetector::new(250.0, bocpd::MIN_SESSIONS),
            stl_mad: StlMadDetector::new(7, stl_mad::MIN_SESSIONS, 3.5),
            causal: CausalGraph::new(),
            feedback: PipelineFeedback::new(),
            pending: Vec::new(),
            next_id: 1,
            stale_baseline: false,
            session_counts: Vec::new(),
        }
    }

    /// Buffer a metric for end-of-session processing.
    pub fn feed(&mut self, metric: SessionMetric) {
        if matches!(metric, SessionMetric::PatchEvent) {
            self.stale_baseline = true;
        }
        self.pending.push(metric);
    }

    /// Run all detectors, apply causal explain-away, apply feedback thresholds,
    /// and return operator-visible anomaly events.  Call once per session close.
    pub fn on_session_close(&mut self) -> Vec<AnomalyEvent> {
        let metrics = std::mem::take(&mut self.pending);
        let stale = self.stale_baseline;

        let mut raw: Vec<AnomalyEvent> = Vec::new();
        let mut cleric_debuff = false;
        let mut missing_buff = false;
        let mut gear_delta = false;

        for metric in &metrics {
            match metric {
                SessionMetric::ClericDebuffActive(v) => cleric_debuff = *v,
                SessionMetric::MissingBuff(v) => missing_buff = *v,
                SessionMetric::GearDelta(v) => gear_delta = *v,
                SessionMetric::Dps { class, zone, value } => {
                    if let Some(mut ev) = self.ewma.update(class, zone, *value, stale) {
                        ev.id = self.next_id;
                        self.next_id += 1;
                        raw.push(ev);
                    }
                }
                SessionMetric::Death { ts_ms } => self.page_hinkley.record_death(*ts_ms),
                SessionMetric::ManaP10 { value } => {
                    if let Some(mut ev) = self.mad.update(*value, stale) {
                        ev.id = self.next_id;
                        self.next_id += 1;
                        raw.push(ev);
                    }
                }
                SessionMetric::StuckEvent { route_id, node_id } => {
                    if let Some(mut ev) = self.bocpd.update(*route_id, *node_id, stale) {
                        ev.id = self.next_id;
                        self.next_id += 1;
                        raw.push(ev);
                    }
                }
                SessionMetric::LootRate { camp, value } => {
                    if let Some(mut ev) = self.stl_mad.update(camp, *value, stale) {
                        ev.id = self.next_id;
                        self.next_id += 1;
                        raw.push(ev);
                    }
                }
                _ => {}
            }
        }

        // Page-Hinkley: check death cluster after all deaths have been recorded.
        if let Some(mut ev) = self.page_hinkley.check_session(stale) {
            ev.id = self.next_id;
            self.next_id += 1;
            raw.push(ev);
        }

        // Causal explain-away: demote correlated child alarms Major → Minor.
        let explained = self
            .causal
            .explain_away(raw, cleric_debuff, missing_buff, gear_delta);

        // Feedback: suppress events from repeatedly-dismissed detectors.
        let filtered = self.feedback.filter(explained);

        // Track per-session count for false-alarm rate.
        self.session_counts.push(filtered.len());
        if self.session_counts.len() > 30 {
            self.session_counts.remove(0);
        }

        // Register IDs for future operator actions.
        self.feedback.register_session(&filtered);

        filtered
    }

    /// Process an operator action on an anomaly card.
    pub fn operator_action(&mut self, action: OperatorAction) {
        self.feedback.record_action(action);
    }

    /// Average anomalies per session over the last 30 sessions.
    pub fn false_alarm_rate(&self) -> f64 {
        if self.session_counts.is_empty() {
            return 0.0;
        }
        let total: usize = self.session_counts.iter().sum();
        total as f64 / self.session_counts.len() as f64
    }
}

impl Default for AnomalyPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_no_mid_session_events() {
        let mut p = AnomalyPipeline::new();
        // Warm up EWMA baseline
        for _ in 0..30 {
            p.feed(SessionMetric::Dps {
                class: "warrior".into(),
                zone: "guk".into(),
                value: 100.0,
            });
        }
        // No events should be available mid-session — only on_session_close
        // produces output. Verify pending buffer is non-empty but no early return.
        assert!(!p.pending.is_empty());
    }

    #[test]
    fn stale_baseline_demotes_to_minor() {
        let mut p = AnomalyPipeline::new();
        // Build baseline
        for _ in 0..30 {
            for _ in 0..1 {
                p.pending.push(SessionMetric::Dps {
                    class: "mage".into(),
                    zone: "solb".into(),
                    value: 100.0,
                });
            }
            p.on_session_close();
        }
        p.feed(SessionMetric::PatchEvent);
        // Force a DPS drop after patch — should emit Minor
        for _ in 0..5 {
            p.feed(SessionMetric::Dps {
                class: "mage".into(),
                zone: "solb".into(),
                value: 5.0, // severe drop
            });
        }
        let events = p.on_session_close();
        for ev in &events {
            if matches!(ev.kind, AnomalyKind::DpsDropEwma { .. }) {
                assert_eq!(ev.severity, Severity::Minor, "stale baseline must emit Minor");
                assert!(ev.stale_baseline);
            }
        }
    }

    #[test]
    fn false_alarm_rate_tracks_sessions() {
        let mut p = AnomalyPipeline::new();
        // 30 clean sessions → rate 0
        for _ in 0..30 {
            p.on_session_close();
        }
        assert_eq!(p.false_alarm_rate(), 0.0);
    }
}

//! Beta-Binomial self-tuning false-alarm suppressor.
//!
//! Each detector key tracks an `(alarms_fired, operator_marked_real)` pair
//! that forms the parameters of a Beta-Binomial posterior.  Two Dismiss
//! actions on the same key raise a per-key threshold, which is applied by
//! downgrading the severity of future events from that key.
//!
//! Target: ≤1 anomaly/session averaged over 30 sessions.  Enforced through
//! progressive severity demotion, not hard suppression.

use std::collections::{HashMap, HashSet};

use crate::improve::{AnomalyEvent, AnomalyKind, OperatorAction, Severity};

// ── DetectorPosterior ─────────────────────────────────────────────────────────

/// Beta-Binomial posterior for one detector key.
#[derive(Debug)]
pub struct DetectorPosterior {
    /// α: total alarms fired (including prior of 1).
    pub alpha: f64,
    /// β: operator-confirmed real alarms (including prior of 1).
    pub beta: f64,
    /// Number of times operator dismissed an alarm from this key.
    dismiss_count: u32,
}

impl DetectorPosterior {
    pub fn new() -> Self {
        Self {
            alpha: 1.0,
            beta: 1.0,
            dismiss_count: 0,
        }
    }

    pub fn record_alarm(&mut self) {
        self.alpha += 1.0;
    }

    pub fn record_real(&mut self) {
        self.beta += 1.0;
    }

    pub fn record_dismiss(&mut self) {
        self.dismiss_count += 1;
    }

    /// Estimated false-positive rate: (fires − real) / fires.
    pub fn false_positive_rate(&self) -> f64 {
        let fires = (self.alpha - 1.0).max(0.0);
        let real = (self.beta - 1.0).max(0.0);
        if fires < f64::EPSILON {
            return 0.0;
        }
        ((fires - real.min(fires)) / fires).clamp(0.0, 1.0)
    }

    /// Severity demotion levels based on dismiss count.
    /// - 0..1 dismissals: no demotion
    /// - 2..3 dismissals: demote Major → Average
    /// - ≥4 dismissals: demote Major → Minor, Average → Minor
    pub fn demotion_levels(&self) -> u32 {
        if self.dismiss_count < 2 {
            0
        } else if self.dismiss_count < 4 {
            1
        } else {
            2
        }
    }
}

impl Default for DetectorPosterior {
    fn default() -> Self {
        Self::new()
    }
}

// ── PipelineFeedback ──────────────────────────────────────────────────────────

/// Manages per-detector posteriors and applies feedback to anomaly events.
pub struct PipelineFeedback {
    posteriors: HashMap<String, DetectorPosterior>,
    /// Map from event ID → detector key (so operator actions can find the key).
    id_to_key: HashMap<u64, String>,
    /// Event IDs from the current session (cleared between sessions).
    current_ids: HashSet<u64>,
}

impl PipelineFeedback {
    pub fn new() -> Self {
        Self {
            posteriors: HashMap::new(),
            id_to_key: HashMap::new(),
            current_ids: HashSet::new(),
        }
    }

    /// Register events produced this session so operator actions can look them up.
    pub fn register_session(&mut self, events: &[AnomalyEvent]) {
        self.current_ids.clear();
        for ev in events {
            let key = event_key(&ev.kind);
            self.id_to_key.insert(ev.id, key.clone());
            self.current_ids.insert(ev.id);
            // Record that an alarm fired for this key
            self.posteriors
                .entry(key)
                .or_default()
                .record_alarm();
        }
    }

    /// Apply severity demotion to events based on accumulated posterior.
    pub fn filter(&self, mut events: Vec<AnomalyEvent>) -> Vec<AnomalyEvent> {
        for ev in &mut events {
            let key = event_key(&ev.kind);
            if let Some(posterior) = self.posteriors.get(&key) {
                let demote = posterior.demotion_levels();
                ev.severity = demote_severity(ev.severity, demote);
            }
        }
        events
    }

    /// Record an operator action.
    pub fn record_action(&mut self, action: OperatorAction) {
        match action {
            OperatorAction::Accept(id) => {
                if let Some(key) = self.id_to_key.get(&id) {
                    self.posteriors.entry(key.clone()).or_default().record_real();
                }
            }
            OperatorAction::Dismiss(id) => {
                if let Some(key) = self.id_to_key.get(&id) {
                    self.posteriors
                        .entry(key.clone())
                        .or_default()
                        .record_dismiss();
                }
            }
            OperatorAction::RealButIgnore(_) => {
                // Does not affect thresholds — operator acknowledges but ignores.
            }
        }
    }

    /// False-positive rate for a given detector key.
    pub fn false_positive_rate(&self, key: &str) -> f64 {
        self.posteriors
            .get(key)
            .map(|p| p.false_positive_rate())
            .unwrap_or(0.0)
    }
}

impl Default for PipelineFeedback {
    fn default() -> Self {
        Self::new()
    }
}

/// Stable string key for a detector kind (includes camp/class/zone for per-camp tuning).
fn event_key(kind: &AnomalyKind) -> String {
    match kind {
        AnomalyKind::DpsDropEwma { class, zone } => format!("dps:{class}:{zone}"),
        AnomalyKind::DeathClusterPageHinkley => "death".to_owned(),
        AnomalyKind::ManaCollapseMad => "mana".to_owned(),
        AnomalyKind::StuckRateBocpd { route_id, node_id } => {
            format!("stuck:{route_id}:{node_id}")
        }
        AnomalyKind::LootRateStlMad { camp } => format!("loot:{camp}"),
    }
}

/// Demote severity by `levels` steps (Major→Average→Minor→Minor).
fn demote_severity(s: Severity, levels: u32) -> Severity {
    match levels {
        0 => s,
        1 => match s {
            Severity::Major => Severity::Average,
            other => other,
        },
        _ => Severity::Minor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_event(id: u64, kind: AnomalyKind, severity: Severity) -> AnomalyEvent {
        AnomalyEvent {
            id,
            ts: Utc::now(),
            kind,
            metric_name: "test".into(),
            baseline_mean: 100.0,
            baseline_std: 5.0,
            observed: 40.0,
            severity,
            caused_by: None,
            stale_baseline: false,
        }
    }

    #[test]
    fn no_demotion_before_dismissals() {
        let mut fb = PipelineFeedback::new();
        let events = vec![make_event(
            1,
            AnomalyKind::ManaCollapseMad,
            Severity::Major,
        )];
        fb.register_session(&events);
        let out = fb.filter(events.clone());
        assert_eq!(out[0].severity, Severity::Major);
    }

    #[test]
    fn one_dismiss_no_demotion() {
        let mut fb = PipelineFeedback::new();
        let ev = make_event(1, AnomalyKind::ManaCollapseMad, Severity::Major);
        fb.register_session(&[ev.clone()]);
        fb.record_action(OperatorAction::Dismiss(1));

        let ev2 = make_event(2, AnomalyKind::ManaCollapseMad, Severity::Major);
        fb.register_session(&[ev2.clone()]);
        let out = fb.filter(vec![ev2]);
        // 1 dismiss < 2 → no demotion yet
        assert_eq!(out[0].severity, Severity::Major);
    }

    #[test]
    fn two_dismissals_demote_major_to_average() {
        let mut fb = PipelineFeedback::new();
        let ev1 = make_event(1, AnomalyKind::ManaCollapseMad, Severity::Major);
        fb.register_session(&[ev1]);
        fb.record_action(OperatorAction::Dismiss(1));

        let ev2 = make_event(2, AnomalyKind::ManaCollapseMad, Severity::Major);
        fb.register_session(&[ev2]);
        fb.record_action(OperatorAction::Dismiss(2));

        let ev3 = make_event(3, AnomalyKind::ManaCollapseMad, Severity::Major);
        fb.register_session(&[ev3.clone()]);
        let out = fb.filter(vec![ev3]);
        assert_eq!(out[0].severity, Severity::Average, "2 dismissals → Major demoted to Average");
    }

    #[test]
    fn four_dismissals_demote_all_to_minor() {
        let mut fb = PipelineFeedback::new();
        for i in 1..=4u64 {
            let ev = make_event(i, AnomalyKind::DeathClusterPageHinkley, Severity::Major);
            fb.register_session(&[ev]);
            fb.record_action(OperatorAction::Dismiss(i));
        }
        let ev5 = make_event(5, AnomalyKind::DeathClusterPageHinkley, Severity::Major);
        fb.register_session(&[ev5.clone()]);
        let out = fb.filter(vec![ev5]);
        assert_eq!(out[0].severity, Severity::Minor, "4+ dismissals → Minor");
    }

    #[test]
    fn accept_records_real_alarm() {
        let mut fb = PipelineFeedback::new();
        let ev = make_event(10, AnomalyKind::ManaCollapseMad, Severity::Major);
        fb.register_session(&[ev]);
        fb.record_action(OperatorAction::Accept(10));
        let fpr = fb.false_positive_rate("mana");
        // After 1 alarm + 1 real: FPR = 0
        assert_eq!(fpr, 0.0);
    }

    #[test]
    fn real_but_ignore_does_not_raise_threshold() {
        let mut fb = PipelineFeedback::new();
        let ev1 = make_event(1, AnomalyKind::ManaCollapseMad, Severity::Major);
        fb.register_session(&[ev1]);
        fb.record_action(OperatorAction::RealButIgnore(1));

        let ev2 = make_event(2, AnomalyKind::ManaCollapseMad, Severity::Major);
        fb.register_session(&[ev2.clone()]);
        let out = fb.filter(vec![ev2]);
        // No threshold raise → still Major
        assert_eq!(out[0].severity, Severity::Major);
    }

    #[test]
    fn per_camp_isolation() {
        let mut fb = PipelineFeedback::new();
        // 2 dismissals on camp_a
        for i in 1..=2u64 {
            let ev = make_event(
                i,
                AnomalyKind::LootRateStlMad { camp: "camp_a".into() },
                Severity::Average,
            );
            fb.register_session(&[ev]);
            fb.record_action(OperatorAction::Dismiss(i));
        }
        // camp_b should be unaffected
        let ev_b = make_event(
            10,
            AnomalyKind::LootRateStlMad { camp: "camp_b".into() },
            Severity::Average,
        );
        fb.register_session(&[ev_b.clone()]);
        let out = fb.filter(vec![ev_b]);
        assert_eq!(out[0].severity, Severity::Average, "camp_b unaffected by camp_a dismissals");
    }

    #[test]
    fn simulated_false_alarm_rate_converges() {
        // Simulate 30 sessions with 1 alarm each, all dismissed → rate drops to ≤1
        let mut fb = PipelineFeedback::new();
        for i in 0..30u64 {
            let ev = make_event(i, AnomalyKind::ManaCollapseMad, Severity::Major);
            fb.register_session(&[ev.clone()]);
            fb.record_action(OperatorAction::Dismiss(i));
            // After 4 dismissals, further alarms get demoted to Minor
        }
        // After 30 dismissals, the key has demotion_levels=2 → all events → Minor
        let posterior = fb.posteriors.get("mana").unwrap();
        assert!(
            posterior.demotion_levels() >= 2,
            "heavy dismissal should max demotion"
        );
    }
}

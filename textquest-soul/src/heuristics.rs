//! Tier-1 rule-based heuristic engine for gameplay anti-pattern detection.
//!
//! Detects common gameplay issues: excessive downtime, mana bottleneck, low pull
//! rate, and camp drift. Surfaces as actionable suggestions to the operator.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use textquest_common::types::ClientId;

// ── Pattern detection types ──────────────────────────────────────────────────

/// Actionable suggestion tier based on pattern severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SuggestionTier {
    /// Minor optimization, not blocking gameplay.
    Minor = 0,
    /// Notable inefficiency that reduces farming rate.
    Average = 1,
    /// Major issue that blocks proper gameplay.
    Major = 2,
}

/// The discriminated kind of pattern detected by tier-1 heuristics.
#[derive(Debug, Clone, PartialEq)]
pub enum HeuristicPattern {
    /// Character idle/no combat for longer than threshold (seconds).
    ExcessiveDowntime { duration_secs: u32 },
    /// Character mana below threshold (percentage).
    ManaBotleneck { mana_percent: u32 },
    /// Pull rate (mobs/minute) below target threshold.
    LowPullRate { current_rate: f32, threshold: f32 },
    /// Character position drifted from camp location (distance units).
    CampDrift { distance_units: f32 },
}

/// A single heuristic suggestion produced by the pattern detector.
#[derive(Debug, Clone)]
pub struct HeuristicSuggestion {
    pub character_id: ClientId,
    pub pattern: HeuristicPattern,
    pub tier: SuggestionTier,
    pub message: String,
    pub timestamp: Instant,
}

// ── Per-character tracking state ─────────────────────────────────────────────

#[derive(Debug, Clone)]
struct CharacterMetrics {
    /// Time of last combat action (pull, cast, attack).
    last_combat_at: Instant,
    /// Current mana percentage (0-100).
    mana_percent: u32,
    /// Current character position (x, y, z).
    position: (f32, f32, f32),
    /// Camp origin point (x, y, z) — set on first observation or explicit reset.
    camp_origin: (f32, f32, f32),
    /// Ring buffer of pull timestamps (last 60 seconds) for pull-rate calculation.
    pull_times: Vec<Instant>,
}

impl CharacterMetrics {
    fn new(position: (f32, f32, f32)) -> Self {
        Self {
            last_combat_at: Instant::now(),
            mana_percent: 100,
            position,
            camp_origin: position,
            pull_times: Vec::new(),
        }
    }

    /// Compute pulls per minute from the pull_times ring buffer.
    fn pulls_per_minute(&self) -> f32 {
        let now = Instant::now();
        let recent_pulls = self
            .pull_times
            .iter()
            .filter(|&&t| now.duration_since(t) < Duration::from_secs(60))
            .count();
        recent_pulls as f32
    }

    /// Compute distance from current position to camp origin.
    fn distance_from_camp(&self) -> f32 {
        let (dx, dy, dz) = (
            self.position.0 - self.camp_origin.0,
            self.position.1 - self.camp_origin.1,
            self.position.2 - self.camp_origin.2,
        );
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

// ── Tier-1 Heuristic Engine ──────────────────────────────────────────────────

/// Detects tier-1 gameplay anti-patterns and returns actionable suggestions.
///
/// Called periodically (e.g., once per coordinator tick) to evaluate each
/// character's metrics and emit suggestions as needed.
pub struct Tier1HeuristicEngine {
    characters: HashMap<ClientId, CharacterMetrics>,

    // Configuration thresholds (public for testing)
    pub downtime_threshold: Duration,
    pub downtime_alert_severity: SuggestionTier,
    pub mana_threshold: u32,
    pub mana_alert_severity: SuggestionTier,
    pub pull_rate_threshold: f32,
    pub pull_rate_alert_severity: SuggestionTier,
    pub camp_drift_threshold: f32,
    pub camp_drift_alert_severity: SuggestionTier,
}

impl Default for Tier1HeuristicEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Tier1HeuristicEngine {
    /// Create an engine with production defaults.
    pub fn new() -> Self {
        Self {
            characters: HashMap::new(),
            downtime_threshold: Duration::from_secs(120),
            downtime_alert_severity: SuggestionTier::Average,
            mana_threshold: 20,
            mana_alert_severity: SuggestionTier::Major,
            pull_rate_threshold: 0.5, // 0.5 mobs/minute
            pull_rate_alert_severity: SuggestionTier::Average,
            camp_drift_threshold: 50.0, // distance units
            camp_drift_alert_severity: SuggestionTier::Minor,
        }
    }

    /// Register a character and set its initial position.
    pub fn register_character(&mut self, character_id: ClientId, position: (f32, f32, f32)) {
        self.characters
            .insert(character_id, CharacterMetrics::new(position));
    }

    /// Update character metrics after a combat action (pull, cast, attack).
    pub fn record_combat_action(&mut self, character_id: ClientId) {
        if let Some(metrics) = self.characters.get_mut(&character_id) {
            metrics.last_combat_at = Instant::now();
        }
    }

    /// Update character mana percentage.
    pub fn update_mana(&mut self, character_id: ClientId, mana_percent: u32) {
        if let Some(metrics) = self.characters.get_mut(&character_id) {
            metrics.mana_percent = mana_percent.min(100);
        }
    }

    /// Update character position (x, y, z).
    pub fn update_position(&mut self, character_id: ClientId, position: (f32, f32, f32)) {
        if let Some(metrics) = self.characters.get_mut(&character_id) {
            metrics.position = position;
        }
    }

    /// Record a successful pull event.
    pub fn record_pull(&mut self, character_id: ClientId) {
        if let Some(metrics) = self.characters.get_mut(&character_id) {
            metrics.pull_times.push(Instant::now());
            // Prune old pull records (older than 60 seconds)
            let now = Instant::now();
            metrics
                .pull_times
                .retain(|&t| now.duration_since(t) < Duration::from_secs(60));
        }
    }

    /// Explicitly set the camp origin for a character (e.g., at session start).
    pub fn set_camp_origin(&mut self, character_id: ClientId, position: (f32, f32, f32)) {
        if let Some(metrics) = self.characters.get_mut(&character_id) {
            metrics.camp_origin = position;
        }
    }

    /// Run heuristic checks and return any triggered suggestions.
    ///
    /// Should be called once per coordinator tick.
    pub fn check(&self) -> Vec<HeuristicSuggestion> {
        let now = Instant::now();
        let mut suggestions = Vec::new();

        for (&character_id, metrics) in &self.characters {
            // 1. Excessive downtime check
            let idle_duration = now.duration_since(metrics.last_combat_at);
            if idle_duration >= self.downtime_threshold {
                suggestions.push(HeuristicSuggestion {
                    character_id,
                    pattern: HeuristicPattern::ExcessiveDowntime {
                        duration_secs: idle_duration.as_secs() as u32,
                    },
                    tier: self.downtime_alert_severity,
                    message: format!(
                        "Character {} has been idle for {:.0}s (threshold: {}s)",
                        character_id,
                        idle_duration.as_secs_f32(),
                        self.downtime_threshold.as_secs()
                    ),
                    timestamp: now,
                });
            }

            // 2. Mana bottleneck check
            if metrics.mana_percent <= self.mana_threshold {
                suggestions.push(HeuristicSuggestion {
                    character_id,
                    pattern: HeuristicPattern::ManaBotleneck {
                        mana_percent: metrics.mana_percent,
                    },
                    tier: self.mana_alert_severity,
                    message: format!(
                        "Character {} mana at {}% (threshold: {}%)",
                        character_id, metrics.mana_percent, self.mana_threshold
                    ),
                    timestamp: now,
                });
            }

            // 3. Low pull rate check
            let pull_rate = metrics.pulls_per_minute();
            if pull_rate < self.pull_rate_threshold {
                suggestions.push(HeuristicSuggestion {
                    character_id,
                    pattern: HeuristicPattern::LowPullRate {
                        current_rate: pull_rate,
                        threshold: self.pull_rate_threshold,
                    },
                    tier: self.pull_rate_alert_severity,
                    message: format!(
                        "Character {} pull rate {:.2} mobs/min (threshold: {:.2})",
                        character_id, pull_rate, self.pull_rate_threshold
                    ),
                    timestamp: now,
                });
            }

            // 4. Camp drift check
            let drift = metrics.distance_from_camp();
            if drift > self.camp_drift_threshold {
                suggestions.push(HeuristicSuggestion {
                    character_id,
                    pattern: HeuristicPattern::CampDrift {
                        distance_units: drift,
                    },
                    tier: self.camp_drift_alert_severity,
                    message: format!(
                        "Character {} drifted {:.1} units from camp (threshold: {:.1})",
                        character_id, drift, self.camp_drift_threshold
                    ),
                    timestamp: now,
                });
            }
        }

        suggestions
    }
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration as StdDuration;

    use super::*;

    fn make_engine() -> Tier1HeuristicEngine {
        Tier1HeuristicEngine::new()
    }

    // ── Excessive downtime ───────────────────────────────────────────────────

    #[test]
    fn downtime_no_alert_when_recent_combat() {
        let mut engine = make_engine();
        engine.register_character(1, (0.0, 0.0, 0.0));
        engine.record_combat_action(1); // just now
        let suggestions = engine.check();
        assert!(
            suggestions
                .iter()
                .all(|s| !matches!(s.pattern, HeuristicPattern::ExcessiveDowntime { .. })),
            "should not alert for recent combat"
        );
    }

    #[test]
    fn downtime_alert_after_threshold() {
        let mut engine = make_engine();
        engine.downtime_threshold = Duration::from_millis(10);
        engine.register_character(2, (0.0, 0.0, 0.0));
        thread::sleep(StdDuration::from_millis(50));
        let suggestions = engine.check();
        let downtime: Vec<_> = suggestions
            .iter()
            .filter(|s| matches!(s.pattern, HeuristicPattern::ExcessiveDowntime { .. }))
            .collect();
        assert_eq!(downtime.len(), 1, "expected one downtime suggestion");
        assert_eq!(downtime[0].tier, SuggestionTier::Average);
    }

    // ── Mana bottleneck ──────────────────────────────────────────────────────

    #[test]
    fn mana_no_alert_above_threshold() {
        let mut engine = make_engine();
        engine.register_character(3, (0.0, 0.0, 0.0));
        engine.update_mana(3, 50);
        let suggestions = engine.check();
        assert!(
            suggestions
                .iter()
                .all(|s| !matches!(s.pattern, HeuristicPattern::ManaBotleneck { .. })),
            "should not alert for mana above threshold"
        );
    }

    #[test]
    fn mana_alert_below_threshold() {
        let mut engine = make_engine();
        engine.register_character(4, (0.0, 0.0, 0.0));
        engine.update_mana(4, 15);
        let suggestions = engine.check();
        let mana: Vec<_> = suggestions
            .iter()
            .filter(|s| matches!(s.pattern, HeuristicPattern::ManaBotleneck { .. }))
            .collect();
        assert_eq!(mana.len(), 1, "expected one mana suggestion");
        assert_eq!(mana[0].tier, SuggestionTier::Major);
    }

    // ── Pull rate ────────────────────────────────────────────────────────────

    #[test]
    fn pull_rate_no_alert_above_threshold() {
        let mut engine = make_engine();
        engine.pull_rate_threshold = 1.0; // 1 pull/minute
        engine.register_character(5, (0.0, 0.0, 0.0));
        // Record 3 pulls in the last 60 seconds → 3 pulls/min > 1 pull/min
        for _ in 0..3 {
            engine.record_pull(5);
        }
        let suggestions = engine.check();
        assert!(
            suggestions
                .iter()
                .all(|s| !matches!(s.pattern, HeuristicPattern::LowPullRate { .. })),
            "should not alert for good pull rate"
        );
    }

    #[test]
    fn pull_rate_alert_below_threshold() {
        let mut engine = make_engine();
        engine.pull_rate_threshold = 2.0; // 2 pulls/minute
        engine.register_character(6, (0.0, 0.0, 0.0));
        engine.record_pull(6); // only 1 pull → below threshold
        let suggestions = engine.check();
        let rate: Vec<_> = suggestions
            .iter()
            .filter(|s| matches!(s.pattern, HeuristicPattern::LowPullRate { .. }))
            .collect();
        assert_eq!(rate.len(), 1, "expected one pull-rate suggestion");
        assert_eq!(rate[0].tier, SuggestionTier::Average);
    }

    // ── Camp drift ───────────────────────────────────────────────────────────

    #[test]
    fn camp_drift_no_alert_near_origin() {
        let mut engine = make_engine();
        engine.register_character(7, (0.0, 0.0, 0.0));
        engine.update_position(7, (10.0, 10.0, 0.0)); // ~14 units from origin
        let suggestions = engine.check();
        assert!(
            suggestions
                .iter()
                .all(|s| !matches!(s.pattern, HeuristicPattern::CampDrift { .. })),
            "should not alert for small drift"
        );
    }

    #[test]
    fn camp_drift_alert_far_from_origin() {
        let mut engine = make_engine();
        engine.camp_drift_threshold = 30.0;
        engine.register_character(8, (0.0, 0.0, 0.0));
        engine.update_position(8, (100.0, 100.0, 0.0)); // ~141 units from origin
        let suggestions = engine.check();
        let drift: Vec<_> = suggestions
            .iter()
            .filter(|s| matches!(s.pattern, HeuristicPattern::CampDrift { .. }))
            .collect();
        assert_eq!(drift.len(), 1, "expected one camp-drift suggestion");
        assert_eq!(drift[0].tier, SuggestionTier::Minor);
    }
}

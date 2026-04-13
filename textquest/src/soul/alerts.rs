//! Operator alerts and anomaly detection for the Soul Engine.
//!
//! `AnomalyDetector` runs on each coordinator tick and surfaces operator-visible alerts
//! for conditions that indicate the Soul Engine is misbehaving or resource-constrained.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use textquest_common::types::ClientId;

// ── Alert types ──────────────────────────────────────────────────────────────

/// Severity level of an alert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Low,
    Medium,
    High,
}

/// Discriminated kind of anomaly detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertType {
    /// No `SoulEvent` received for the character in over 60 seconds.
    StuckCharacter,
    /// Mood changed more than 5 times within the last 60 seconds.
    MoodOscillation,
    /// Persistent memory record count exceeded 500 for a character.
    MemoryOverflow,
    /// LLM token quota has been exhausted (budget == 0 and requests are pending).
    LlmQuotaExhausted,
}

/// A single operator alert produced by `AnomalyDetector`.
#[derive(Debug, Clone)]
pub struct Alert {
    pub character_id: ClientId,
    pub alert_type: AlertType,
    pub severity: Severity,
    pub message: String,
    pub timestamp: Instant,
}

// ── Per-character tracking state ─────────────────────────────────────────────

#[derive(Debug)]
struct CharacterState {
    /// Time of the last soul event received.
    last_event_at: Instant,
    /// Ring buffer of mood-change timestamps used for oscillation detection.
    mood_change_times: Vec<Instant>,
    /// Approximate memory record count (updated by the coordinator).
    memory_count: usize,
}

impl CharacterState {
    fn new() -> Self {
        Self {
            last_event_at: Instant::now(),
            mood_change_times: Vec::new(),
            memory_count: 0,
        }
    }
}

// ── AnomalyDetector ───────────────────────────────────────────────────────────

/// Detects operational anomalies in the Soul Engine and returns operator alerts.
///
/// Intended to be called once per coordinator tick.  Results are surfaced to the
/// TUI operator dashboard and, optionally, to the Discord webhook.
pub struct AnomalyDetector {
    characters: HashMap<ClientId, CharacterState>,
    /// If `true`, the LLM quota is exhausted (budget == 0, requests pending).
    llm_quota_exhausted: bool,
    /// Number of LLM requests waiting when quota was exhausted.
    llm_pending_count: usize,

    // Configuration knobs (public so tests can override)
    pub stuck_threshold: Duration,
    pub mood_osc_window: Duration,
    pub mood_osc_max_changes: usize,
    pub memory_overflow_threshold: usize,
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl AnomalyDetector {
    /// Create a detector with production defaults.
    pub fn new() -> Self {
        Self {
            characters: HashMap::new(),
            llm_quota_exhausted: false,
            llm_pending_count: 0,
            stuck_threshold: Duration::from_secs(60),
            mood_osc_window: Duration::from_secs(60),
            mood_osc_max_changes: 5,
            memory_overflow_threshold: 500,
        }
    }

    /// Notify the detector that a soul event was received for a character.
    /// Call this whenever `SoulCoordinator::on_game_event` is called.
    pub fn record_soul_event(&mut self, character_id: ClientId) {
        self.characters
            .entry(character_id)
            .or_insert_with(CharacterState::new)
            .last_event_at = Instant::now();
    }

    /// Notify the detector that a character's mood changed.
    pub fn record_mood_change(&mut self, character_id: ClientId) {
        let state = self
            .characters
            .entry(character_id)
            .or_insert_with(CharacterState::new);
        state.mood_change_times.push(Instant::now());
    }

    /// Update the approximate memory record count for a character.
    pub fn update_memory_count(&mut self, character_id: ClientId, count: usize) {
        self.characters
            .entry(character_id)
            .or_insert_with(CharacterState::new)
            .memory_count = count;
    }

    /// Inform the detector about the current LLM quota state.
    ///
    /// `budget_remaining` — token budget left this period.
    /// `pending_count`    — number of requests waiting in the queue.
    pub fn update_llm_state(&mut self, budget_remaining: u64, pending_count: usize) {
        self.llm_quota_exhausted = budget_remaining == 0 && pending_count > 0;
        self.llm_pending_count = pending_count;
    }

    /// Register a character so the detector can track it from the start.
    pub fn register_character(&mut self, character_id: ClientId) {
        self.characters
            .entry(character_id)
            .or_insert_with(CharacterState::new);
    }

    /// Run anomaly checks and return any active alerts.
    ///
    /// Should be called once per coordinator tick.
    pub fn check(&mut self) -> Vec<Alert> {
        let now = Instant::now();
        let mut alerts = Vec::new();

        for (&character_id, state) in &mut self.characters {
            // 1. Stuck-character check
            let elapsed = now.duration_since(state.last_event_at);
            if elapsed >= self.stuck_threshold {
                alerts.push(Alert {
                    character_id,
                    alert_type: AlertType::StuckCharacter,
                    severity: Severity::High,
                    message: format!(
                        "Character {} has received no soul events for {:.0}s",
                        character_id,
                        elapsed.as_secs_f32()
                    ),
                    timestamp: now,
                });
            }

            // 2. Mood-oscillation check — prune old timestamps, then count
            let window = self.mood_osc_window;
            state
                .mood_change_times
                .retain(|&t| now.duration_since(t) < window);
            if state.mood_change_times.len() > self.mood_osc_max_changes {
                alerts.push(Alert {
                    character_id,
                    alert_type: AlertType::MoodOscillation,
                    severity: Severity::Medium,
                    message: format!(
                        "Character {} had {} mood changes in the last {}s (threshold: {})",
                        character_id,
                        state.mood_change_times.len(),
                        self.mood_osc_window.as_secs(),
                        self.mood_osc_max_changes,
                    ),
                    timestamp: now,
                });
            }

            // 3. Memory-overflow check
            if state.memory_count > self.memory_overflow_threshold {
                alerts.push(Alert {
                    character_id,
                    alert_type: AlertType::MemoryOverflow,
                    severity: Severity::Medium,
                    message: format!(
                        "Character {} memory record count {} exceeds threshold {}",
                        character_id, state.memory_count, self.memory_overflow_threshold
                    ),
                    timestamp: now,
                });
            }
        }

        // 4. LLM quota check (global, not per-character)
        if self.llm_quota_exhausted {
            // Use ClientId 0 as a sentinel for a global / non-character alert
            alerts.push(Alert {
                character_id: 0,
                alert_type: AlertType::LlmQuotaExhausted,
                severity: Severity::High,
                message: format!(
                    "LLM token quota exhausted — {} request(s) pending, budget is 0",
                    self.llm_pending_count
                ),
                timestamp: now,
            });
        }

        alerts
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn make_detector() -> AnomalyDetector {
        AnomalyDetector::new()
    }

    // Helper: force last_event_at into the past by mutating the state map.
    fn age_last_event(detector: &mut AnomalyDetector, character_id: ClientId, age: Duration) {
        if let Some(state) = detector.characters.get_mut(&character_id) {
            // Subtract `age` from the stored instant via a mock re-assignment.
            // `Instant` arithmetic: we can't subtract from an Instant directly, so
            // we push mood_change_times to an old timestamp via the public API, but
            // for last_event_at we use a workaround: set it to `Instant::now() - age`
            // which is guaranteed to exist (instants never underflow past the OS epoch
            // for any `age` under a day).
            state.last_event_at = Instant::now() - age;
        }
    }

    // ── 1. Stuck character ───────────────────────────────────────────────────

    #[test]
    fn stuck_character_no_alert_when_recent() {
        let mut d = make_detector();
        d.register_character(1);
        d.record_soul_event(1); // just now
        let alerts = d.check();
        assert!(
            alerts
                .iter()
                .all(|a| a.alert_type != AlertType::StuckCharacter),
            "should not alert when event is recent"
        );
    }

    #[test]
    fn stuck_character_alert_after_threshold() {
        let mut d = make_detector();
        d.stuck_threshold = Duration::from_millis(10); // short threshold for test
        d.register_character(2);
        age_last_event(&mut d, 2, Duration::from_millis(100));
        let alerts = d.check();
        let stuck: Vec<_> = alerts
            .iter()
            .filter(|a| a.alert_type == AlertType::StuckCharacter && a.character_id == 2)
            .collect();
        assert_eq!(stuck.len(), 1, "expected exactly one StuckCharacter alert");
        assert_eq!(stuck[0].severity, Severity::High);
    }

    // ── 2. Mood oscillation ──────────────────────────────────────────────────

    #[test]
    fn mood_oscillation_no_alert_below_threshold() {
        let mut d = make_detector();
        d.register_character(3);
        // Record 5 changes — exactly at the limit (not exceeding)
        for _ in 0..5 {
            d.record_mood_change(3);
        }
        let alerts = d.check();
        assert!(
            alerts
                .iter()
                .all(|a| a.alert_type != AlertType::MoodOscillation),
            "exactly 5 changes should not exceed the 5-change threshold"
        );
    }

    #[test]
    fn mood_oscillation_alert_above_threshold() {
        let mut d = make_detector();
        d.register_character(4);
        // Record 6 changes — exceeds the default threshold of 5
        for _ in 0..6 {
            d.record_mood_change(4);
        }
        let alerts = d.check();
        let osc: Vec<_> = alerts
            .iter()
            .filter(|a| a.alert_type == AlertType::MoodOscillation && a.character_id == 4)
            .collect();
        assert_eq!(osc.len(), 1, "expected exactly one MoodOscillation alert");
        assert_eq!(osc[0].severity, Severity::Medium);
    }

    // ── 3. Memory overflow ───────────────────────────────────────────────────

    #[test]
    fn memory_overflow_no_alert_at_threshold() {
        let mut d = make_detector();
        d.register_character(5);
        d.update_memory_count(5, 500); // exactly at limit — should not fire
        let alerts = d.check();
        assert!(
            alerts
                .iter()
                .all(|a| a.alert_type != AlertType::MemoryOverflow),
            "count == threshold should not alert"
        );
    }

    #[test]
    fn memory_overflow_alert_above_threshold() {
        let mut d = make_detector();
        d.register_character(6);
        d.update_memory_count(6, 501); // one above the limit
        let alerts = d.check();
        let overflow: Vec<_> = alerts
            .iter()
            .filter(|a| a.alert_type == AlertType::MemoryOverflow && a.character_id == 6)
            .collect();
        assert_eq!(
            overflow.len(),
            1,
            "expected exactly one MemoryOverflow alert"
        );
        assert_eq!(overflow[0].severity, Severity::Medium);
    }

    // ── 4. LLM quota exhaustion ──────────────────────────────────────────────

    #[test]
    fn llm_quota_no_alert_when_budget_available() {
        let mut d = make_detector();
        d.update_llm_state(1000, 5); // budget > 0, so no alert
        let alerts = d.check();
        assert!(
            alerts
                .iter()
                .all(|a| a.alert_type != AlertType::LlmQuotaExhausted),
            "should not alert when budget is non-zero"
        );
    }

    #[test]
    fn llm_quota_alert_when_exhausted_with_pending() {
        let mut d = make_detector();
        d.update_llm_state(0, 3); // budget == 0, 3 pending
        let alerts = d.check();
        let quota: Vec<_> = alerts
            .iter()
            .filter(|a| a.alert_type == AlertType::LlmQuotaExhausted)
            .collect();
        assert_eq!(
            quota.len(),
            1,
            "expected exactly one LlmQuotaExhausted alert"
        );
        assert_eq!(quota[0].severity, Severity::High);
        assert_eq!(quota[0].character_id, 0); // global sentinel
    }

    #[test]
    fn llm_quota_no_alert_when_budget_zero_but_no_pending() {
        let mut d = make_detector();
        d.update_llm_state(0, 0); // budget == 0 but nothing pending — no alert
        let alerts = d.check();
        assert!(
            alerts
                .iter()
                .all(|a| a.alert_type != AlertType::LlmQuotaExhausted),
            "should not alert when budget is zero but no requests are pending"
        );
    }
}

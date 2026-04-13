//! Operator controls and safety mechanisms for the Soul Engine.
//!
//! Provides a kill-switch, per-character suppression, alert history, and chat-rate
//! thresholds so an operator can quickly quell unwanted behavior across all managed
//! characters without restarting the process.

use std::collections::{HashSet, VecDeque};

// ---------------------------------------------------------------------------
// Alert types
// ---------------------------------------------------------------------------

/// Categories of Soul Engine operator alerts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoulAlertType {
    /// A character exceeded the configured chat-rate threshold.
    ExcessiveChatRate,
    /// A character produced output that fell outside expected behavioral bounds.
    UnexpectedBehavior,
    /// The Soul Engine's LLM request queue has grown beyond healthy levels.
    QueueBackpressure,
    /// The operator kill-switch was activated.
    KillSwitchTriggered,
}

/// A single operator-visible alert emitted by the Soul Engine.
#[derive(Debug, Clone)]
pub struct SoulAlert {
    /// The character that triggered this alert (or `"<system>"` for system-level alerts).
    pub character: String,
    /// Category of this alert.
    pub alert_type: SoulAlertType,
    /// Human-readable description of the event.
    pub message: String,
    /// Monotonic instant at which the alert was recorded, for elapsed-time calculations.
    pub timestamp: std::time::SystemTime,
}

impl SoulAlert {
    /// Convenience constructor.
    pub fn new(character: impl Into<String>, alert_type: SoulAlertType, message: impl Into<String>) -> Self {
        Self {
            character: character.into(),
            alert_type,
            message: message.into(),
            timestamp: std::time::SystemTime::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Operator controls
// ---------------------------------------------------------------------------

/// Maximum number of alerts retained in `SoulOperatorControls::alert_history`.
const MAX_ALERT_HISTORY: usize = 50;

/// Operator-facing safety controls for the Soul Engine.
///
/// All Soul Engine subsystems should call [`SoulOperatorControls::is_allowed`]
/// before producing output for a character.  Operators can:
///
/// * Toggle a **global kill-switch** that immediately blocks all characters.
/// * **Suppress** individual characters without affecting the rest of the group.
/// * Inspect an **alert history** of recent safety events.
/// * Configure a **chat-rate threshold** (chats per minute) used by monitoring code
///   to emit [`SoulAlertType::ExcessiveChatRate`] alerts.
#[derive(Debug)]
pub struct SoulOperatorControls {
    /// When `true`, no Soul Engine output is permitted for any character.
    pub kill_switch_active: bool,
    /// Characters whose Soul Engine output is individually suppressed.
    suppressed_characters: HashSet<String>,
    /// Ring buffer of the most recent operator alerts (capped at 50).
    alert_history: VecDeque<SoulAlert>,
    /// Maximum number of in-game chat messages per minute before an
    /// [`SoulAlertType::ExcessiveChatRate`] alert is raised.
    pub chat_rate_threshold: u32,
}

impl Default for SoulOperatorControls {
    fn default() -> Self {
        Self::new()
    }
}

impl SoulOperatorControls {
    /// Create a new `SoulOperatorControls` with sensible defaults.
    ///
    /// * Kill-switch: inactive
    /// * No characters suppressed
    /// * Alert history: empty
    /// * Chat-rate threshold: 10 messages per minute
    pub fn new() -> Self {
        Self {
            kill_switch_active: false,
            suppressed_characters: HashSet::new(),
            alert_history: VecDeque::new(),
            chat_rate_threshold: 10,
        }
    }

    // ------------------------------------------------------------------
    // Kill-switch
    // ------------------------------------------------------------------

    /// Activate the global kill-switch.
    ///
    /// Sets `kill_switch_active` to `true` and appends a single
    /// [`SoulAlertType::KillSwitchTriggered`] system-level alert with
    /// `"<system>"` as the alert character so the alert history reflects the
    /// activation event.
    pub fn activate_kill_switch(&mut self) {
        self.kill_switch_active = true;

        // Record a single system-level alert capturing the activation event.
        let alert = SoulAlert::new(
            "<system>",
            SoulAlertType::KillSwitchTriggered,
            "Operator kill-switch activated — all Soul Engine output suspended",
        );
        self.record_alert(alert);
    }

    /// Deactivate the global kill-switch, resuming normal operation.
    pub fn deactivate_kill_switch(&mut self) {
        self.kill_switch_active = false;
    }

    // ------------------------------------------------------------------
    // Per-character suppression
    // ------------------------------------------------------------------

    /// Suppress Soul Engine output for `character`.
    pub fn suppress_character(&mut self, character: &str) {
        self.suppressed_characters.insert(character.to_owned());
    }

    /// Lift the suppression for `character`, re-enabling Soul Engine output.
    pub fn unsuppress_character(&mut self, character: &str) {
        self.suppressed_characters.remove(character);
    }

    // ------------------------------------------------------------------
    // Gate check
    // ------------------------------------------------------------------

    /// Returns `true` if the Soul Engine is allowed to produce output for
    /// `character`.
    ///
    /// Output is blocked when either:
    /// * The global kill-switch is active, or
    /// * The character is individually suppressed.
    pub fn is_allowed(&self, character: &str) -> bool {
        !self.kill_switch_active && !self.suppressed_characters.contains(character)
    }

    // ------------------------------------------------------------------
    // Alert management
    // ------------------------------------------------------------------

    /// Append `alert` to the history, evicting the oldest entry when the
    /// history exceeds `MAX_ALERT_HISTORY` (50).
    pub fn record_alert(&mut self, alert: SoulAlert) {
        if self.alert_history.len() >= MAX_ALERT_HISTORY {
            self.alert_history.pop_front();
        }
        self.alert_history.push_back(alert);
    }

    /// Return up to the last `limit` alerts in chronological order (oldest
    /// first among the returned slice).
    pub fn recent_alerts(&self, limit: usize) -> Vec<&SoulAlert> {
        let skip = self.alert_history.len().saturating_sub(limit);
        self.alert_history.iter().skip(skip).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_alert(character: &str, alert_type: SoulAlertType) -> SoulAlert {
        SoulAlert::new(character, alert_type, "test alert")
    }

    // --- kill-switch ---

    #[test]
    fn kill_switch_blocks_all_characters() {
        let mut controls = SoulOperatorControls::new();
        controls.activate_kill_switch();
        assert!(controls.kill_switch_active);
        assert!(!controls.is_allowed("Warrior"));
        assert!(!controls.is_allowed("Cleric"));
        assert!(!controls.is_allowed("Rogue"));
    }

    #[test]
    fn deactivate_kill_switch_restores_access() {
        let mut controls = SoulOperatorControls::new();
        controls.activate_kill_switch();
        controls.deactivate_kill_switch();
        assert!(!controls.kill_switch_active);
        assert!(controls.is_allowed("Warrior"));
    }

    #[test]
    fn kill_switch_generates_alert() {
        let mut controls = SoulOperatorControls::new();
        controls.activate_kill_switch();
        assert!(!controls.alert_history.is_empty());
        let alert = controls.alert_history.back().unwrap();
        assert_eq!(alert.alert_type, SoulAlertType::KillSwitchTriggered);
    }

    // --- suppress / unsuppress ---

    #[test]
    fn suppress_blocks_specific_character() {
        let mut controls = SoulOperatorControls::new();
        controls.suppress_character("Rogue");
        assert!(!controls.is_allowed("Rogue"));
        assert!(controls.is_allowed("Cleric"));
    }

    #[test]
    fn unsuppress_restores_character() {
        let mut controls = SoulOperatorControls::new();
        controls.suppress_character("Rogue");
        controls.unsuppress_character("Rogue");
        assert!(controls.is_allowed("Rogue"));
    }

    #[test]
    fn unsuppress_nonexistent_is_noop() {
        let mut controls = SoulOperatorControls::new();
        controls.unsuppress_character("Ghost");
        assert!(controls.is_allowed("Ghost"));
    }

    // --- is_allowed logic ---

    #[test]
    fn is_allowed_true_by_default() {
        let controls = SoulOperatorControls::new();
        assert!(controls.is_allowed("Warrior"));
    }

    #[test]
    fn is_allowed_false_when_kill_switch_and_suppressed() {
        let mut controls = SoulOperatorControls::new();
        controls.suppress_character("Rogue");
        controls.activate_kill_switch();
        // Both conditions block — result is still false.
        assert!(!controls.is_allowed("Rogue"));
    }

    // --- alert history cap ---

    #[test]
    fn alert_history_caps_at_50() {
        let mut controls = SoulOperatorControls::new();
        for i in 0..60u32 {
            controls.record_alert(make_alert(
                &format!("Char{i}"),
                SoulAlertType::ExcessiveChatRate,
            ));
        }
        assert_eq!(controls.alert_history.len(), 50);
    }

    #[test]
    fn alert_history_evicts_oldest() {
        let mut controls = SoulOperatorControls::new();
        for i in 0..50u32 {
            controls.record_alert(make_alert(
                &format!("Char{i}"),
                SoulAlertType::ExcessiveChatRate,
            ));
        }
        // Add one more — "Char0" should be evicted.
        controls.record_alert(make_alert("CharNew", SoulAlertType::UnexpectedBehavior));
        assert_eq!(controls.alert_history.len(), 50);
        let front = controls.alert_history.front().unwrap();
        assert_eq!(front.character, "Char1");
    }

    // --- recent_alerts ---

    #[test]
    fn recent_alerts_returns_last_n() {
        let mut controls = SoulOperatorControls::new();
        for i in 0..10u32 {
            controls.record_alert(make_alert(
                &format!("Char{i}"),
                SoulAlertType::QueueBackpressure,
            ));
        }
        let recent = controls.recent_alerts(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].character, "Char7");
        assert_eq!(recent[1].character, "Char8");
        assert_eq!(recent[2].character, "Char9");
    }

    #[test]
    fn recent_alerts_limit_larger_than_history() {
        let mut controls = SoulOperatorControls::new();
        controls.record_alert(make_alert("Char0", SoulAlertType::ExcessiveChatRate));
        let recent = controls.recent_alerts(100);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn recent_alerts_empty_history() {
        let controls = SoulOperatorControls::new();
        assert!(controls.recent_alerts(5).is_empty());
    }
}

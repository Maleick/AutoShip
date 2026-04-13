//! Auto-forage automation — periodically fires `/forage` via IPC.
//!
//! `ForageManager` tracks interval timing, skill-use counts, and a capped
//! ring buffer of recent forage result strings. It does NOT hook into the
//! live camp loop; call `tick` from whatever driving context needs it.

#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::too_long_first_doc_paragraph
)]

use std::sync::mpsc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use textquest_common::ipc::{Command, IpcCommand};

// ── Configuration ────────────────────────────────────────────────────────────

/// Serialisable configuration for `ForageManager`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForageConfig {
    /// Enable or disable auto-foraging.
    pub enabled: bool,
    /// Minimum milliseconds between `/forage` attempts.
    #[serde(default = "ForageConfig::default_interval_ms")]
    pub interval_ms: u64,
    /// Maximum number of forage result strings to keep in history.
    #[serde(default = "ForageConfig::default_max_results_history")]
    pub max_results_history: usize,
}

impl ForageConfig {
    fn default_interval_ms() -> u64 {
        3_000
    }

    fn default_max_results_history() -> usize {
        50
    }
}

impl Default for ForageConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_ms: Self::default_interval_ms(),
            max_results_history: Self::default_max_results_history(),
        }
    }
}

// ── Manager ──────────────────────────────────────────────────────────────────

/// Drives the auto-forage loop for one character.
///
/// Call `tick` on every orchestrator tick (or similar cadence). The manager
/// sends `/forage` via IPC whenever `enabled` is true and at least
/// `interval_ms` have elapsed since the last attempt.
pub struct ForageManager {
    /// Whether auto-foraging is active.
    pub enabled: bool,
    /// Milliseconds to wait between `/forage` commands.
    pub interval_ms: u64,
    /// Instant of the most recent forage attempt, or `None` if never fired.
    pub last_forage_tick: Option<Instant>,
    /// Total number of `/forage` commands sent this session.
    pub skill_uses: u64,
    /// Ring buffer of recent forage result messages (capped at `max_results_history`).
    pub results: Vec<String>,
    /// Maximum number of entries kept in `results`.
    max_results_history: usize,
}

impl ForageManager {
    /// Create a new manager from a `ForageConfig`.
    #[must_use]
    pub fn new(config: &ForageConfig) -> Self {
        Self {
            enabled: config.enabled,
            interval_ms: config.interval_ms,
            last_forage_tick: None,
            skill_uses: 0,
            results: Vec::new(),
            max_results_history: config.max_results_history,
        }
    }

    /// Record a forage result string (e.g., from game chat parsing).
    ///
    /// Trims the history to `max_results_history` entries.
    pub fn record_result(&mut self, result: String) {
        self.results.push(result);
        if self.results.len() > self.max_results_history {
            let excess = self.results.len() - self.max_results_history;
            self.results.drain(..excess);
        }
    }

    /// Drive the forage interval.
    ///
    /// Returns `Some(log_message)` when a `/forage` command is sent, `None`
    /// when the interval has not elapsed or the manager is disabled.
    ///
    /// The `ipc_tx` channel carries `IpcCommand` values to the IPC dispatch
    /// layer. A send failure is silently ignored (the caller can observe it via
    /// the absent log message on the next tick).
    pub fn tick(&mut self, now: Instant, ipc_tx: &mpsc::Sender<IpcCommand>) -> Option<String> {
        if !self.enabled {
            return None;
        }

        let should_fire = match self.last_forage_tick {
            None => true,
            Some(last) => now.duration_since(last).as_millis() >= u128::from(self.interval_ms),
        };

        if !should_fire {
            return None;
        }

        let cmd = IpcCommand::new(Command::SlashCommand {
            command: "/forage".to_owned(),
        });

        // Best-effort send; ignore channel errors (receiver may not be wired
        // in tests or when the camp loop is paused).
        let _ = ipc_tx.send(cmd);

        self.last_forage_tick = Some(now);
        self.skill_uses += 1;

        Some(format!("forage: sent /forage (use #{})", self.skill_uses))
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn default_manager() -> ForageManager {
        ForageManager::new(&ForageConfig::default())
    }

    fn enabled_manager() -> ForageManager {
        ForageManager::new(&ForageConfig {
            enabled: true,
            interval_ms: 1_000,
            max_results_history: 5,
        })
    }

    // Helper: create a connected channel pair.
    fn make_channel() -> (mpsc::Sender<IpcCommand>, mpsc::Receiver<IpcCommand>) {
        mpsc::channel()
    }

    // ── disabled manager ─────────────────────────────────────────────────────

    #[test]
    fn test_disabled_manager_never_sends() {
        let mut mgr = default_manager(); // enabled = false
        assert!(!mgr.enabled);

        let (tx, rx) = make_channel();
        let now = Instant::now();

        let result = mgr.tick(now, &tx);
        assert!(result.is_none(), "disabled manager should return None");
        assert!(
            rx.try_recv().is_err(),
            "disabled manager should not send IPC"
        );
        assert_eq!(mgr.skill_uses, 0);
    }

    // ── interval gating ──────────────────────────────────────────────────────

    #[test]
    fn test_first_tick_fires_immediately() {
        let mut mgr = enabled_manager();
        let (tx, rx) = make_channel();
        let now = Instant::now();

        let result = mgr.tick(now, &tx);
        assert!(result.is_some(), "first tick should fire");
        assert!(rx.try_recv().is_ok(), "IPC command should be sent");
        assert_eq!(mgr.skill_uses, 1);
    }

    #[test]
    fn test_second_tick_before_interval_skips() {
        let mut mgr = enabled_manager(); // interval_ms = 1000
        let (tx, rx) = make_channel();
        let t0 = Instant::now();

        // First tick — fires.
        mgr.tick(t0, &tx);
        let _ = rx.try_recv();

        // Immediately after — must NOT fire.
        let t1 = t0 + Duration::from_millis(500);
        let result = mgr.tick(t1, &tx);
        assert!(
            result.is_none(),
            "tick before interval elapsed should be skipped"
        );
        assert!(rx.try_recv().is_err(), "no IPC command within interval");
        assert_eq!(mgr.skill_uses, 1);
    }

    #[test]
    fn test_tick_fires_after_interval_elapsed() {
        let mut mgr = enabled_manager(); // interval_ms = 1000
        let (tx, rx) = make_channel();
        let t0 = Instant::now();

        // First tick.
        mgr.tick(t0, &tx);
        let _ = rx.try_recv();

        // After full interval — must fire.
        let t2 = t0 + Duration::from_millis(1_001);
        let result = mgr.tick(t2, &tx);
        assert!(result.is_some(), "tick after interval should fire");
        assert!(rx.try_recv().is_ok(), "second IPC command sent");
        assert_eq!(mgr.skill_uses, 2);
    }

    #[test]
    fn test_no_double_fire_at_exact_boundary() {
        let mut mgr = enabled_manager(); // interval_ms = 1000
        let (tx, rx) = make_channel();
        let t0 = Instant::now();

        mgr.tick(t0, &tx);
        let _ = rx.try_recv();

        // Exactly at interval boundary — should fire (>=).
        let t_boundary = t0 + Duration::from_millis(1_000);
        let result = mgr.tick(t_boundary, &tx);
        assert!(result.is_some(), "tick at exact boundary should fire");
        let _ = rx.try_recv();

        // One millisecond after firing again — should NOT fire.
        let t_just_after = t_boundary + Duration::from_millis(1);
        let result2 = mgr.tick(t_just_after, &tx);
        assert!(
            result2.is_none(),
            "no double-fire right after boundary tick"
        );
        assert!(rx.try_recv().is_err());
    }

    // ── results history ──────────────────────────────────────────────────────

    #[test]
    fn test_results_history_caps_at_max() {
        let mut mgr = enabled_manager(); // max_results_history = 5
        for i in 0..10 {
            mgr.record_result(format!("result {i}"));
        }
        assert_eq!(
            mgr.results.len(),
            5,
            "history must be capped at max_results_history"
        );
        // Newest entries should be retained.
        assert_eq!(mgr.results[0], "result 5");
        assert_eq!(mgr.results[4], "result 9");
    }

    #[test]
    fn test_results_history_does_not_overflow_below_max() {
        let mut mgr = enabled_manager(); // max_results_history = 5
        for i in 0..3 {
            mgr.record_result(format!("result {i}"));
        }
        assert_eq!(mgr.results.len(), 3);
    }

    // ── IPC command content ──────────────────────────────────────────────────

    #[test]
    fn test_ipc_command_is_slash_forage() {
        let mut mgr = enabled_manager();
        let (tx, rx) = make_channel();
        let now = Instant::now();

        mgr.tick(now, &tx);
        let cmd = rx.try_recv().expect("command should be present");

        match cmd.command {
            Command::SlashCommand { command } => {
                assert_eq!(command, "/forage");
            }
            other => panic!("expected SlashCommand, got {other:?}"),
        }
    }

    // ── ForageConfig defaults ────────────────────────────────────────────────

    #[test]
    fn test_forage_config_defaults() {
        let cfg = ForageConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.interval_ms, 3_000);
        assert_eq!(cfg.max_results_history, 50);
    }

    #[test]
    fn test_forage_config_serde_roundtrip() {
        let cfg = ForageConfig {
            enabled: true,
            interval_ms: 2_500,
            max_results_history: 20,
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        let decoded: ForageConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cfg, decoded);
    }

    // ── skill_uses counter ───────────────────────────────────────────────────

    #[test]
    fn test_skill_uses_increments_each_fire() {
        let mut mgr = enabled_manager(); // interval_ms = 1000
        let (tx, rx) = make_channel();
        let t0 = Instant::now();

        for i in 1..=3u64 {
            let t = t0 + Duration::from_millis(i * 1_001);
            mgr.tick(t, &tx);
            let _ = rx.try_recv();
        }
        assert_eq!(mgr.skill_uses, 3);
    }
}

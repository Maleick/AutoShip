//! Crash reporting and session recovery for EQ client processes.
//!
//! Captures per-character context snapshots and provides recovery command
//! sequences when a client crashes or becomes unresponsive.

use std::collections::HashMap;
use std::time::SystemTime;

/// A snapshot of the known good state for a single EQ client.
#[derive(Debug, Clone)]
pub struct CrashContext {
    /// OS process ID of the EQ client.
    pub pid: u32,
    /// Character name associated with this client.
    pub character: String,
    /// Zone the character was in when the snapshot was taken.
    pub zone: String,
    /// Last known state string (e.g. "combat", "idle", "navigating").
    pub last_state: String,
    /// Wall-clock time the snapshot was recorded.
    pub timestamp: SystemTime,
    /// Tail of the log at snapshot time (up to the last 20 lines).
    pub log_tail: Vec<String>,
}

/// Tracks per-character crash contexts and produces recovery actions.
#[derive(Debug, Default)]
pub struct CrashReporter {
    contexts: HashMap<String, CrashContext>,
}

impl CrashReporter {
    /// Creates a new, empty `CrashReporter`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores (or replaces) the latest context for the character named in `ctx`.
    pub fn record_context(&mut self, ctx: CrashContext) {
        self.contexts.insert(ctx.character.clone(), ctx);
    }

    /// Returns the last recorded context for `character`, or `None` if unknown.
    #[must_use]
    pub fn last_context(&self, character: &str) -> Option<&CrashContext> {
        self.contexts.get(character)
    }

    /// Returns the default recovery command sequence for `character`.
    ///
    /// If a context exists the sequence is `["/camp desktop"]`; otherwise empty.
    #[must_use]
    pub fn recovery_commands(&self, character: &str) -> Vec<String> {
        if self.contexts.contains_key(character) {
            vec!["/camp desktop".to_string()]
        } else {
            vec![]
        }
    }

    /// Formats the stored context for `character` as a human-readable string.
    ///
    /// Returns `None` if no context has been recorded for that character.
    #[must_use]
    pub fn format_report(&self, character: &str) -> Option<String> {
        let ctx = self.contexts.get(character)?;

        let timestamp_secs = ctx
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let log_tail = if ctx.log_tail.is_empty() {
            "(no log lines captured)".to_string()
        } else {
            ctx.log_tail.join("\n    ")
        };

        Some(format!(
            "=== Crash Report: {} ===\nPID:        {}\nCharacter:  {}\nZone:       {}\nLast State: {}\nTimestamp:  {} (unix secs)\nLog Tail:\n    {}",
            ctx.character,
            ctx.pid,
            ctx.character,
            ctx.zone,
            ctx.last_state,
            timestamp_secs,
            log_tail,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn make_ctx(character: &str) -> CrashContext {
        CrashContext {
            pid: 1234,
            character: character.to_string(),
            zone: "East Commonlands".to_string(),
            last_state: "combat".to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            log_tail: vec![
                "line1".to_string(),
                "line2".to_string(),
                "line3".to_string(),
            ],
        }
    }

    #[test]
    fn record_context_stores_per_character() {
        let mut reporter = CrashReporter::new();
        let ctx_a = make_ctx("Kira");
        let ctx_b = make_ctx("Maldrek");

        reporter.record_context(ctx_a);
        reporter.record_context(ctx_b);

        assert!(reporter.on_crash("Kira").is_some());
        assert!(reporter.on_crash("Maldrek").is_some());
        assert!(reporter.on_crash("Unknown").is_none());
    }

    #[test]
    fn record_context_overwrites_previous() {
        let mut reporter = CrashReporter::new();
        reporter.record_context(make_ctx("Kira"));

        let mut updated = make_ctx("Kira");
        updated.zone = "Kithicor Forest".to_string();
        reporter.record_context(updated);

        let ctx = reporter.on_crash("Kira").expect("context should exist");
        assert_eq!(ctx.zone, "Kithicor Forest");
    }

    #[test]
    fn on_crash_returns_context() {
        let mut reporter = CrashReporter::new();
        let ctx = make_ctx("Kira");
        reporter.record_context(ctx);

        let found = reporter.on_crash("Kira").expect("should find context");
        assert_eq!(found.character, "Kira");
        assert_eq!(found.pid, 1234);
        assert_eq!(found.zone, "East Commonlands");
        assert_eq!(found.last_state, "combat");
    }

    #[test]
    fn on_crash_missing_character_returns_none() {
        let reporter = CrashReporter::new();
        assert!(reporter.on_crash("NoSuchToon").is_none());
    }

    #[test]
    fn recovery_commands_returns_default_sequence() {
        let mut reporter = CrashReporter::new();
        reporter.record_context(make_ctx("Kira"));

        let cmds = reporter.recovery_commands("Kira");
        assert_eq!(cmds, vec!["/camp desktop"]);
    }

    #[test]
    fn recovery_commands_empty_for_unknown_character() {
        let reporter = CrashReporter::new();
        let cmds = reporter.recovery_commands("Ghost");
        assert!(cmds.is_empty());
    }

    #[test]
    fn format_report_includes_key_fields() {
        let mut reporter = CrashReporter::new();
        reporter.record_context(make_ctx("Kira"));

        let report = reporter.format_report("Kira").expect("report should exist");
        assert!(report.contains("Kira"), "report should include character name");
        assert!(report.contains("1234"), "report should include pid");
        assert!(report.contains("East Commonlands"), "report should include zone");
        assert!(report.contains("combat"), "report should include last_state");
        assert!(report.contains("line1"), "report should include log tail");
    }

    #[test]
    fn format_report_missing_character_returns_none() {
        let reporter = CrashReporter::new();
        assert!(reporter.format_report("Nobody").is_none());
    }
}

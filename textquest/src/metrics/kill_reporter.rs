//! Kill tracker auto-reporting — formats and sends periodic kill statistics
//! to configured chat channels (MQ2KillTracker parity).
//!
//! Auto-reports are formatted for in-game chat channels (group, raid, guild, etc.)
//! and include session summaries, kills/hour, and optional per-mob breakdowns.

use crate::config::KillTrackerConfig;
use crate::metrics::{KillSessionStore, KillTracker};

pub struct KillReporter {
    config: KillTrackerConfig,
    last_report_time: Option<std::time::Instant>,
}

impl KillReporter {
    pub fn new(config: KillTrackerConfig) -> Self {
        Self {
            config,
            last_report_time: None,
        }
    }

    pub fn should_report(&self, elapsed: std::time::Duration) -> bool {
        if !self.config.enabled || self.config.auto_report_interval_minutes == 0 {
            return false;
        }
        let interval = std::time::Duration::from_secs(
            u64::from(self.config.auto_report_interval_minutes) * 60,
        );
        if let Some(last) = self.last_report_time {
            last.elapsed() >= interval
        } else {
            elapsed >= interval
        }
    }

    pub fn mark_reported(&mut self) {
        self.last_report_time = Some(std::time::Instant::now());
    }

    pub fn format_report(
        &self,
        tracker: &KillTracker,
        _session_store: &KillSessionStore,
        character: &str,
        zone: &str,
        elapsed_secs: u64,
    ) -> String {
        let total_kills = tracker.total_kills() as u32;
        let total_deaths = tracker.total_deaths();
        let mob_stats = tracker.mob_stats();
        let top_mobs = tracker.top_mobs(5);
        let efficiency = tracker.efficiency(chrono::Utc::now().timestamp());

        let hours = elapsed_secs as f64 / 3600.0;
        let kills_per_hour = if hours > 0.0 {
            total_kills as f64 / hours
        } else {
            0.0
        };

        let mut lines = vec![format!(
            "[KillTracker] Session Report for {} in {} ({:.1}h elapsed)",
            character, zone, hours
        )];

        lines.push(format!(
            "Kills: {} | Deaths: {} | K/D: {:.2} | KPH: {:.1}",
            total_kills,
            total_deaths,
            if total_deaths > 0 {
                total_kills as f64 / total_deaths as f64
            } else {
                total_kills as f64
            },
            kills_per_hour
        ));

        if self.config.auto_report_include_mobs && !top_mobs.is_empty() {
            lines.push(String::from("Top mobs:"));
            for (mob_name, count) in top_mobs.iter().take(5) {
                if let Some(stats) = mob_stats.get(mob_name) {
                    let pct = if total_kills > 0 {
                        (stats.kill_count as f64 / total_kills as f64 * 100.0) as u32
                    } else {
                        0
                    };
                    lines.push(format!("  {} - {} ({}%)", mob_name, count, pct));
                }
            }
        }

        if self.config.auto_report_include_kph {
            lines.push(format!(
                "Efficiency Score: {:.0}/100 ({:.1} KPH avg)",
                efficiency.score, efficiency.kills_per_hour
            ));
        }

        lines.join("\n")
    }

    pub fn chat_command_for_channel(&self) -> String {
        let channel = self.config.auto_report_channel.as_str();
        match channel {
            "group" => "/g".to_string(),
            "raid" => "/ra".to_string(),
            "guild" => "/gu".to_string(),
            "say" => "/s".to_string(),
            "shout" => "/shout".to_string(),
            "ooc" => "/ooc".to_string(),
            _ => "/g".to_string(),
        }
    }

    pub fn config(&self) -> &KillTrackerConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: KillTrackerConfig) {
        self.config = config;
    }
}

impl Default for KillReporter {
    fn default() -> Self {
        Self::new(KillTrackerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(mob: &str, ts: i64) -> crate::metrics::KillRecord {
        crate::metrics::KillRecord {
            mob_name: mob.to_string(),
            mob_level: 5,
            zone: "gfaydark".to_string(),
            killer_pid: 1000,
            kill_time_ms: 10_000,
            total_damage: 5000,
            timestamp: ts,
        }
    }

    #[test]
    fn should_report_disabled() {
        let config = KillTrackerConfig {
            enabled: false,
            auto_report_interval_minutes: 10,
            auto_report_channel: "group".to_string(),
            auto_report_include_mobs: true,
            auto_report_include_kph: true,
            track_per_character: true,
            max_session_history: 100,
        };
        let reporter = KillReporter::new(config);
        assert!(!reporter.should_report(std::time::Duration::from_secs(600)));
    }

    #[test]
    fn should_report_interval_zero_disabled() {
        let config = KillTrackerConfig {
            enabled: true,
            auto_report_interval_minutes: 0,
            auto_report_channel: "group".to_string(),
            auto_report_include_mobs: true,
            auto_report_include_kph: true,
            track_per_character: true,
            max_session_history: 100,
        };
        let reporter = KillReporter::new(config);
        assert!(!reporter.should_report(std::time::Duration::from_secs(1000)));
    }

    #[test]
    fn should_report_after_interval() {
        let config = KillTrackerConfig {
            enabled: true,
            auto_report_interval_minutes: 10,
            auto_report_channel: "group".to_string(),
            auto_report_include_mobs: true,
            auto_report_include_kph: true,
            track_per_character: true,
            max_session_history: 100,
        };
        let reporter = KillReporter::new(config);
        // 11 minutes should trigger report
        assert!(reporter.should_report(std::time::Duration::from_secs(660)));
    }

    #[test]
    fn mark_reported() {
        let config = KillTrackerConfig::default();
        let mut reporter = KillReporter::new(config);
        reporter.mark_reported();
        // Immediately after marking, should not report
        assert!(!reporter.should_report(std::time::Duration::from_secs(1)));
    }

    #[test]
    fn chat_command_mapping() {
        let test_cases = vec![
            ("group", "/g"),
            ("raid", "/ra"),
            ("guild", "/gu"),
            ("say", "/s"),
            ("shout", "/shout"),
            ("ooc", "/ooc"),
            ("unknown", "/g"),
        ];

        for (channel, expected_cmd) in test_cases {
            let config = KillTrackerConfig {
                auto_report_channel: channel.to_string(),
                ..KillTrackerConfig::default()
            };
            let reporter = KillReporter::new(config);
            assert_eq!(reporter.chat_command_for_channel(), expected_cmd);
        }
    }

    #[test]
    fn format_report_basic() {
        let config = KillTrackerConfig::default();
        let reporter = KillReporter::new(config);

        let mut tracker = KillTracker::new(chrono::Utc::now().timestamp() - 3600);
        tracker.record_kill(make_record("orc_pawn", chrono::Utc::now().timestamp()));
        tracker.record_kill(make_record("orc_pawn", chrono::Utc::now().timestamp()));
        tracker.record_kill(make_record("goblin", chrono::Utc::now().timestamp()));

        let store = KillSessionStore::new();
        let report = reporter.format_report(&tracker, &store, "TestChar", "gfaydark", 3600);

        assert!(report.contains("TestChar"));
        assert!(report.contains("gfaydark"));
        assert!(report.contains("Kills:"));
        assert!(report.contains("3"));
    }

    #[test]
    fn format_report_top_mobs() {
        let config = KillTrackerConfig {
            auto_report_include_mobs: true,
            ..KillTrackerConfig::default()
        };
        let reporter = KillReporter::new(config);

        let mut tracker = KillTracker::new(chrono::Utc::now().timestamp() - 3600);
        for _ in 0..5 {
            tracker.record_kill(make_record("orc_pawn", chrono::Utc::now().timestamp()));
        }
        for _ in 0..3 {
            tracker.record_kill(make_record("goblin", chrono::Utc::now().timestamp()));
        }
        for _ in 0..2 {
            tracker.record_kill(make_record("skeleton", chrono::Utc::now().timestamp()));
        }

        let store = KillSessionStore::new();
        let report = reporter.format_report(&tracker, &store, "TestChar", "gfaydark", 3600);

        assert!(report.contains("orc_pawn"));
        assert!(report.contains("goblin"));
        assert!(report.contains("Top mobs"));
    }

    #[test]
    fn format_report_empty_session() {
        let config = KillTrackerConfig::default();
        let reporter = KillReporter::new(config);

        let tracker = KillTracker::new(chrono::Utc::now().timestamp());
        let store = KillSessionStore::new();

        let report = reporter.format_report(&tracker, &store, "NewChar", "poknowledge", 0);

        assert!(report.contains("NewChar"));
        assert!(report.contains("Kills: 0"));
        assert!(report.contains("Deaths: 0"));
    }

    #[test]
    fn reporter_default() {
        let reporter = KillReporter::default();
        assert!(reporter.config().enabled);
        assert_eq!(reporter.config().auto_report_interval_minutes, 10);
    }

    #[test]
    fn update_config() {
        let mut reporter = KillReporter::default();
        let new_config = KillTrackerConfig {
            auto_report_interval_minutes: 5,
            auto_report_channel: "raid".to_string(),
            ..KillTrackerConfig::default()
        };
        reporter.update_config(new_config.clone());
        assert_eq!(reporter.config().auto_report_interval_minutes, 5);
        assert_eq!(reporter.config().auto_report_channel, "raid");
    }
}

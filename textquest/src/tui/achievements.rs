//! Achievement system — tracks milestones, raid firsts, and progression
//! unlocks.

use std::time::Duration;

use serde::{Deserialize, Serialize};

// ─── Achievement kinds ──────────────────────────────────────────────────────

/// The category of an achievement — determines unlock criteria and display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AchievementKind {
    /// Accumulated platinum across all characters reaches a threshold.
    PlatinumMilestone(u64),
    /// First kill of a named raid boss (stores boss name).
    RaidBossFirstKill(String),
    /// Completed a raid with zero deaths across the entire box army.
    ZeroDeathRaid,
    /// Cleared a zone/event under a target time (zone name, target duration).
    SpeedRun(String, Duration),
    /// Every character in the group is wearing a full set of plate (or
    /// equivalent).
    FullPlate,
    /// Completed all content in an expansion tier.
    ExpansionComplete,
}

// ─── Achievement ────────────────────────────────────────────────────────────

/// A single trackable achievement with progress and unlock state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    /// What kind of achievement this is.
    pub kind: AchievementKind,
    /// Human-readable name shown in the UI.
    pub name: String,
    /// Longer description / flavor text.
    pub description: String,
    /// When the achievement was unlocked (`None` if still locked).
    pub unlocked_at: Option<String>,
    /// Progress toward completion (0.0 = not started, 1.0 = done).
    pub progress: f32,
}

impl Achievement {
    /// Creates a new locked achievement with zero progress.
    #[must_use]
    pub fn new(
        kind: AchievementKind,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            description: description.into(),
            unlocked_at: None,
            progress: 0.0,
        }
    }

    /// Whether this achievement has been unlocked.
    #[must_use]
    pub fn is_unlocked(&self) -> bool {
        self.unlocked_at.is_some()
    }

    /// Set progress (clamped to 0.0..=1.0).
    pub fn set_progress(&mut self, value: f32) {
        self.progress = value.clamp(0.0, 1.0);
    }
}

// ─── Tracker ────────────────────────────────────────────────────────────────

/// Manages the full collection of achievements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AchievementTracker {
    achievements: Vec<Achievement>,
}

impl AchievementTracker {
    /// Creates an empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new achievement to track.
    pub fn add(&mut self, achievement: Achievement) {
        self.achievements.push(achievement);
    }

    /// Returns a reference to all achievements.
    #[must_use]
    pub fn all(&self) -> &[Achievement] {
        &self.achievements
    }

    /// Returns only unlocked achievements.
    #[must_use]
    pub fn unlocked(&self) -> Vec<&Achievement> {
        self.achievements
            .iter()
            .filter(|a| a.is_unlocked())
            .collect()
    }

    /// Returns only locked (incomplete) achievements.
    #[must_use]
    pub fn locked(&self) -> Vec<&Achievement> {
        self.achievements
            .iter()
            .filter(|a| !a.is_unlocked())
            .collect()
    }

    /// Finds a mutable reference to the first achievement matching the given
    /// kind.
    pub fn find_mut(&mut self, kind: &AchievementKind) -> Option<&mut Achievement> {
        self.achievements.iter_mut().find(|a| &a.kind == kind)
    }

    /// Unlocks the first achievement matching the given kind, setting its
    /// timestamp. Returns `true` if an achievement was found and unlocked,
    /// `false` otherwise.
    pub fn unlock(&mut self, kind: &AchievementKind, timestamp: impl Into<String>) -> bool {
        if let Some(achievement) = self.find_mut(kind)
            && achievement.unlocked_at.is_none()
        {
            achievement.unlocked_at = Some(timestamp.into());
            achievement.progress = 1.0;
            return true;
        }
        false
    }

    /// Updates progress for the first achievement matching the given kind.
    /// Returns `true` if found and updated.
    pub fn update_progress(&mut self, kind: &AchievementKind, progress: f32) -> bool {
        if let Some(achievement) = self.find_mut(kind) {
            achievement.set_progress(progress);
            return true;
        }
        false
    }

    /// Total number of tracked achievements.
    #[must_use]
    pub fn total(&self) -> usize {
        self.achievements.len()
    }

    /// Number of unlocked achievements.
    #[must_use]
    pub fn unlocked_count(&self) -> usize {
        self.achievements.iter().filter(|a| a.is_unlocked()).count()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tracker() -> AchievementTracker {
        let mut tracker = AchievementTracker::new();
        tracker.add(Achievement::new(
            AchievementKind::PlatinumMilestone(1_000_000),
            "Platinum Hoarder",
            "Accumulate 1,000,000 platinum across all characters",
        ));
        tracker.add(Achievement::new(
            AchievementKind::RaidBossFirstKill("Lord Nagafen".into()),
            "Dragon Slayer",
            "Defeat Lord Nagafen for the first time",
        ));
        tracker.add(Achievement::new(
            AchievementKind::ZeroDeathRaid,
            "Flawless Victory",
            "Complete a raid with zero deaths",
        ));
        tracker.add(Achievement::new(
            AchievementKind::SpeedRun("Plane of Fear".into(), Duration::from_secs(1800)),
            "Speed Demon",
            "Clear Plane of Fear in under 30 minutes",
        ));
        tracker.add(Achievement::new(
            AchievementKind::FullPlate,
            "Tin Can Army",
            "Every character wearing a full plate set",
        ));
        tracker.add(Achievement::new(
            AchievementKind::ExpansionComplete,
            "Expansion Master",
            "Complete all content in an expansion",
        ));
        tracker
    }

    #[test]
    fn new_achievement_is_locked() {
        let a = Achievement::new(AchievementKind::ZeroDeathRaid, "Test", "desc");
        assert!(!a.is_unlocked());
        assert_eq!(a.progress, 0.0);
    }

    #[test]
    fn progress_clamps_to_range() {
        let mut a = Achievement::new(AchievementKind::FullPlate, "Test", "desc");
        a.set_progress(1.5);
        assert_eq!(a.progress, 1.0);
        a.set_progress(-0.5);
        assert_eq!(a.progress, 0.0);
        a.set_progress(0.75);
        assert_eq!(a.progress, 0.75);
    }

    #[test]
    fn tracker_starts_empty() {
        let tracker = AchievementTracker::new();
        assert_eq!(tracker.total(), 0);
        assert_eq!(tracker.unlocked_count(), 0);
        assert!(tracker.unlocked().is_empty());
        assert!(tracker.locked().is_empty());
    }

    #[test]
    fn add_and_count() {
        let tracker = sample_tracker();
        assert_eq!(tracker.total(), 6);
        assert_eq!(tracker.unlocked_count(), 0);
        assert_eq!(tracker.locked().len(), 6);
    }

    #[test]
    fn unlock_achievement() {
        let mut tracker = sample_tracker();
        let kind = AchievementKind::ZeroDeathRaid;
        assert!(tracker.unlock(&kind, "2026-04-04T12:00:00Z"));
        assert_eq!(tracker.unlocked_count(), 1);
        assert_eq!(tracker.locked().len(), 5);

        let unlocked = tracker.unlocked();
        assert_eq!(unlocked[0].name, "Flawless Victory");
        assert_eq!(unlocked[0].progress, 1.0);
    }

    #[test]
    fn unlock_already_unlocked_returns_false() {
        let mut tracker = sample_tracker();
        let kind = AchievementKind::ZeroDeathRaid;
        assert!(tracker.unlock(&kind, "2026-04-04T12:00:00Z"));
        assert!(!tracker.unlock(&kind, "2026-04-04T13:00:00Z"));
        assert_eq!(tracker.unlocked_count(), 1);
    }

    #[test]
    fn unlock_nonexistent_kind_returns_false() {
        let mut tracker = sample_tracker();
        let kind = AchievementKind::PlatinumMilestone(999);
        assert!(!tracker.unlock(&kind, "2026-04-04T12:00:00Z"));
    }

    #[test]
    fn update_progress() {
        let mut tracker = sample_tracker();
        let kind = AchievementKind::PlatinumMilestone(1_000_000);
        assert!(tracker.update_progress(&kind, 0.5));

        let a = tracker.find_mut(&kind).unwrap();
        assert_eq!(a.progress, 0.5);
        assert!(!a.is_unlocked());
    }

    #[test]
    fn update_progress_nonexistent_returns_false() {
        let mut tracker = sample_tracker();
        let kind = AchievementKind::PlatinumMilestone(42);
        assert!(!tracker.update_progress(&kind, 0.5));
    }

    #[test]
    fn multiple_unlocks() {
        let mut tracker = sample_tracker();
        tracker.unlock(&AchievementKind::ZeroDeathRaid, "2026-04-04T12:00:00Z");
        tracker.unlock(&AchievementKind::FullPlate, "2026-04-04T12:01:00Z");
        tracker.unlock(&AchievementKind::ExpansionComplete, "2026-04-04T12:02:00Z");
        assert_eq!(tracker.unlocked_count(), 3);
        assert_eq!(tracker.locked().len(), 3);
    }

    #[test]
    fn serde_roundtrip() {
        let mut tracker = sample_tracker();
        tracker.unlock(&AchievementKind::ZeroDeathRaid, "2026-04-04T12:00:00Z");
        tracker.update_progress(&AchievementKind::PlatinumMilestone(1_000_000), 0.75);

        let json = serde_json::to_string(&tracker).unwrap();
        let restored: AchievementTracker = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.total(), tracker.total());
        assert_eq!(restored.unlocked_count(), tracker.unlocked_count());
    }
}

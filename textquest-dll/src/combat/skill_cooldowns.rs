//! Per-skill cooldown tracking for melee abilities.
//!
//! EQ melee skills have individual cooldowns (e.g. taunt ~6s, kick ~7s, bash
//! ~10s). This tracker replaces the old shared 60-tick timer with per-skill
//! timers so skills fire as soon as they come off cooldown rather than all at
//! once.

/// Maximum number of concurrent skill cooldowns tracked.
const MAX_TRACKED_SKILLS: usize = 16;

/// Known EQ skill IDs and their cooldowns in game ticks (~20 ticks/sec).
pub mod skill_timers {
    pub const KICK: (u32, u32) = (30, 140); // ~7s
    pub const BASH: (u32, u32) = (10, 200); // ~10s
    pub const BACKSTAB: (u32, u32) = (8, 200); // ~10s
    pub const TAUNT: (u32, u32) = (73, 120); // ~6s
    pub const FLYING_KICK: (u32, u32) = (26, 140); // ~7s
    pub const ROUND_KICK: (u32, u32) = (38, 140); // ~7s
    pub const TIGER_CLAW: (u32, u32) = (52, 120); // ~6s
    pub const EAGLE_STRIKE: (u32, u32) = (23, 120); // ~6s
}

/// Tracks per-skill cooldown timers using a fixed-capacity array.
/// Each skill has its own independent timer that counts down every tick.
/// Uses linear scan over a small array instead of HashMap for zero-allocation
/// per-tick operation (~8 skills tracked, linear scan faster than hashing).
pub struct SkillCooldownTracker {
    entries: [(u32, u32); MAX_TRACKED_SKILLS],
    len: usize,
}

impl SkillCooldownTracker {
    pub fn new() -> Self {
        Self {
            entries: [(0, 0); MAX_TRACKED_SKILLS],
            len: 0,
        }
    }

    /// Returns true if the skill has no active cooldown (ready to fire).
    #[inline]
    pub fn is_ready(&self, skill_id: u32) -> bool {
        for i in 0..self.len {
            if self.entries[i].0 == skill_id {
                return self.entries[i].1 == 0;
            }
        }
        true
    }

    /// Put a skill on cooldown for the given number of ticks.
    #[inline]
    pub fn consume(&mut self, skill_id: u32, cooldown_ticks: u32) {
        for i in 0..self.len {
            if self.entries[i].0 == skill_id {
                self.entries[i].1 = cooldown_ticks;
                return;
            }
        }
        if self.len < MAX_TRACKED_SKILLS {
            self.entries[self.len] = (skill_id, cooldown_ticks);
            self.len += 1;
        }
    }

    /// Decrement all active cooldown timers by one tick.
    /// Removes entries that have reached zero to keep the array compact.
    #[inline]
    pub fn tick(&mut self) {
        let mut i = 0;
        while i < self.len {
            self.entries[i].1 = self.entries[i].1.saturating_sub(1);
            if self.entries[i].1 == 0 {
                self.len -= 1;
                if i < self.len {
                    self.entries[i] = self.entries[self.len];
                }
            } else {
                i += 1;
            }
        }
    }
}

/// Look up the default cooldown (in ticks) for a known skill ID.
/// Returns `None` for unknown skills.
pub fn default_cooldown(skill_id: u32) -> Option<u32> {
    match skill_id {
        30 => Some(skill_timers::KICK.1),
        10 => Some(skill_timers::BASH.1),
        8 => Some(skill_timers::BACKSTAB.1),
        73 => Some(skill_timers::TAUNT.1),
        26 => Some(skill_timers::FLYING_KICK.1),
        38 => Some(skill_timers::ROUND_KICK.1),
        52 => Some(skill_timers::TIGER_CLAW.1),
        23 => Some(skill_timers::EAGLE_STRIKE.1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_starts_ready() {
        let tracker = SkillCooldownTracker::new();
        assert!(tracker.is_ready(skill_timers::KICK.0));
        assert!(tracker.is_ready(skill_timers::TAUNT.0));
    }

    #[test]
    fn skill_not_ready_after_consume() {
        let mut tracker = SkillCooldownTracker::new();
        tracker.consume(skill_timers::KICK.0, skill_timers::KICK.1);
        assert!(!tracker.is_ready(skill_timers::KICK.0));
    }

    #[test]
    fn skill_ready_after_full_cooldown() {
        let mut tracker = SkillCooldownTracker::new();
        tracker.consume(skill_timers::TAUNT.0, skill_timers::TAUNT.1); // 120 ticks

        for _ in 0..119 {
            tracker.tick();
            assert!(!tracker.is_ready(skill_timers::TAUNT.0));
        }
        tracker.tick(); // tick 120
        assert!(tracker.is_ready(skill_timers::TAUNT.0));
    }

    #[test]
    fn independent_skill_timers() {
        let mut tracker = SkillCooldownTracker::new();
        // Taunt: 120 ticks, Kick: 140 ticks
        tracker.consume(skill_timers::TAUNT.0, skill_timers::TAUNT.1);
        tracker.consume(skill_timers::KICK.0, skill_timers::KICK.1);

        // After 120 ticks: taunt ready, kick not
        for _ in 0..120 {
            tracker.tick();
        }
        assert!(tracker.is_ready(skill_timers::TAUNT.0));
        assert!(!tracker.is_ready(skill_timers::KICK.0));

        // After 20 more ticks (140 total): kick ready too
        for _ in 0..20 {
            tracker.tick();
        }
        assert!(tracker.is_ready(skill_timers::KICK.0));
    }

    #[test]
    fn consume_resets_cooldown() {
        let mut tracker = SkillCooldownTracker::new();
        tracker.consume(skill_timers::KICK.0, skill_timers::KICK.1);

        // Tick down to ready
        for _ in 0..140 {
            tracker.tick();
        }
        assert!(tracker.is_ready(skill_timers::KICK.0));

        // Consume again
        tracker.consume(skill_timers::KICK.0, skill_timers::KICK.1);
        assert!(!tracker.is_ready(skill_timers::KICK.0));
    }

    #[test]
    fn default_cooldown_known_skills() {
        assert_eq!(default_cooldown(30), Some(140)); // kick
        assert_eq!(default_cooldown(10), Some(200)); // bash
        assert_eq!(default_cooldown(73), Some(120)); // taunt
        assert_eq!(default_cooldown(999), None); // unknown
    }

    #[test]
    fn default_cooldown_all_known() {
        assert_eq!(default_cooldown(8), Some(200)); // backstab
        assert_eq!(default_cooldown(26), Some(140)); // flying kick
        assert_eq!(default_cooldown(38), Some(140)); // round kick
        assert_eq!(default_cooldown(52), Some(120)); // tiger claw
        assert_eq!(default_cooldown(23), Some(120)); // eagle strike
    }

    #[test]
    fn tick_with_no_cooldowns_is_noop() {
        let mut tracker = SkillCooldownTracker::new();
        tracker.tick(); // should not panic
        assert!(tracker.is_ready(1));
    }

    #[test]
    fn multiple_skills_tracked_independently() {
        let mut tracker = SkillCooldownTracker::new();
        tracker.consume(1, 10);
        tracker.consume(2, 20);

        for _ in 0..10 {
            tracker.tick();
        }
        assert!(tracker.is_ready(1));
        assert!(!tracker.is_ready(2));

        for _ in 0..10 {
            tracker.tick();
        }
        assert!(tracker.is_ready(2));
    }

    #[test]
    fn consume_zero_cooldown_stays_ready() {
        let mut tracker = SkillCooldownTracker::new();
        tracker.consume(1, 0);
        assert!(tracker.is_ready(1));
    }

    #[test]
    fn saturating_sub_prevents_underflow() {
        let mut tracker = SkillCooldownTracker::new();
        tracker.consume(1, 1);
        tracker.tick(); // goes to 0
        tracker.tick(); // should stay at 0, not underflow
        assert!(tracker.is_ready(1));
    }
}

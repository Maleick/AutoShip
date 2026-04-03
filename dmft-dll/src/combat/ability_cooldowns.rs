//! Cooldown tracking for activated abilities (disciplines, AA-like clicks).
//!
//! Unlike melee skills, these abilities often have long reuse timers and
//! incomplete metadata. This tracker represents known cooldowns as well as
//! "unknown" timers so we can throttle retries instead of spamming the EQ
//! client every tick.

use std::collections::HashMap;

/// Public view of an ability's availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityAvailability {
    /// Ability is ready to be used.
    Ready,
    /// Ability is cooling down with the given ticks remaining.
    CoolingDown(u32),
    /// Cooldown metadata was unavailable; wait until `retry_at` before retrying.
    WaitingForRetry { retry_at: u32 },
}

impl AbilityAvailability {
    /// Whether the ability can be attempted at the given tick.
    pub fn is_ready_at(&self, now: u32) -> bool {
        match self {
            AbilityAvailability::Ready => true,
            AbilityAvailability::CoolingDown(remaining) => *remaining == 0,
            AbilityAvailability::WaitingForRetry { retry_at } => now >= *retry_at,
        }
    }
}

/// Tracks cooldown state for abilities with optional metadata.
pub struct AbilityCooldownTracker {
    states: HashMap<i32, AbilityAvailability>,
    fallback_retry_ticks: u32,
}

impl AbilityCooldownTracker {
    const DEFAULT_RETRY_TICKS: u32 = 120;

    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            fallback_retry_ticks: Self::DEFAULT_RETRY_TICKS,
        }
    }

    #[cfg(test)]
    pub fn with_retry_ticks(fallback_retry_ticks: u32) -> Self {
        Self {
            states: HashMap::new(),
            fallback_retry_ticks,
        }
    }

    /// Advance cooldowns by one tick and drop entries that have expired.
    pub fn tick(&mut self, now: u32) {
        self.states.retain(|_, state| match state {
            AbilityAvailability::CoolingDown(remaining) => {
                *remaining = remaining.saturating_sub(1);
                *remaining > 0
            }
            AbilityAvailability::WaitingForRetry { retry_at } => now < *retry_at,
            AbilityAvailability::Ready => false,
        });
    }

    /// View the current availability state for an ability at the given tick.
    pub fn availability(&self, ability_id: i32, now: u32) -> AbilityAvailability {
        match self.states.get(&ability_id) {
            Some(AbilityAvailability::CoolingDown(remaining)) => {
                if *remaining == 0 {
                    AbilityAvailability::Ready
                } else {
                    AbilityAvailability::CoolingDown(*remaining)
                }
            }
            Some(AbilityAvailability::WaitingForRetry { retry_at }) => {
                if now >= *retry_at {
                    AbilityAvailability::Ready
                } else {
                    AbilityAvailability::WaitingForRetry {
                        retry_at: *retry_at,
                    }
                }
            }
            _ => AbilityAvailability::Ready,
        }
    }

    /// Whether an ability can be attempted at the given tick.
    pub fn can_use(&self, ability_id: i32, now: u32) -> bool {
        self.availability(ability_id, now).is_ready_at(now)
    }

    /// Mark an ability as consumed with a known cooldown, or fall back to a
    /// retry window when metadata is missing or invalid.
    pub fn consume(&mut self, ability_id: i32, cooldown_ticks: Option<u32>, now: u32) {
        let state = match cooldown_ticks {
            Some(ticks) if ticks > 0 => AbilityAvailability::CoolingDown(ticks),
            _ => AbilityAvailability::WaitingForRetry {
                retry_at: now.saturating_add(self.fallback_retry_ticks),
            },
        };
        self.states.insert(ability_id, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_ready() {
        let tracker = AbilityCooldownTracker::new();
        assert!(tracker.can_use(10, 0));
    }

    #[test]
    fn known_cooldown_counts_down_and_expires() {
        let mut tracker = AbilityCooldownTracker::with_retry_ticks(5);
        tracker.consume(42, Some(3), 10);

        // Immediately after consume, the ability is blocked.
        assert!(!tracker.can_use(42, 10));
        tracker.tick(11); // remaining 2
        tracker.tick(12); // remaining 1
        tracker.tick(13); // drop at 0

        assert!(tracker.can_use(42, 13));
        assert_eq!(tracker.availability(42, 13), AbilityAvailability::Ready);
    }

    #[test]
    fn unknown_metadata_uses_retry_backoff() {
        let mut tracker = AbilityCooldownTracker::with_retry_ticks(4);
        tracker.consume(7, None, 100);

        // Before retry window
        for now in 100..103 {
            assert!(!tracker.can_use(7, now));
            assert_eq!(
                tracker.availability(7, now),
                AbilityAvailability::WaitingForRetry { retry_at: 104 }
            );
        }

        // At retry boundary: becomes usable and entry is pruned on tick().
        assert!(tracker.can_use(7, 104));
        tracker.tick(104);
        assert!(tracker.can_use(7, 105));
        assert_eq!(tracker.availability(7, 105), AbilityAvailability::Ready);
    }

    #[test]
    fn zero_cooldown_treated_as_unknown() {
        let mut tracker = AbilityCooldownTracker::with_retry_ticks(2);
        tracker.consume(9, Some(0), 50);
        assert!(!tracker.can_use(9, 50));
        assert_eq!(
            tracker.availability(9, 50),
            AbilityAvailability::WaitingForRetry { retry_at: 52 }
        );
    }
}

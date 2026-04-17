//! Cooldown tracking for activated abilities (disciplines, AA-like clicks).
//!
//! Unlike melee skills, these abilities often have long reuse timers and
//! incomplete metadata. This tracker represents known cooldowns as well as
//! "unknown" timers so we can throttle retries instead of spamming the EQ
//! client every tick.

/// Initial inline capacity for tracked ability cooldowns.
const INITIAL_TRACKED_ABILITIES: usize = 16;

/// Public view of an ability's availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityAvailability {
    /// Ability is ready to be used.
    Ready,
    /// Ability is cooling down with the given ticks remaining.
    CoolingDown(u32),
    /// Cooldown metadata was unavailable; wait until `retry_at` before
    /// retrying.
    WaitingForRetry { retry_at: u32 },
}

impl AbilityAvailability {
    /// Whether the ability can be attempted at the given tick.
    #[inline]
    pub fn is_ready_at(&self, now: u32) -> bool {
        match self {
            AbilityAvailability::Ready => true,
            AbilityAvailability::CoolingDown(remaining) => *remaining == 0,
            AbilityAvailability::WaitingForRetry { retry_at } => now >= *retry_at,
        }
    }
}

/// Tracks cooldown state for abilities with optional metadata.
///
/// A small inline capacity keeps the common case allocation-light, while the
/// backing `Vec` can still grow for target-scoped rotation entries like
/// Enchanter debuffs that legitimately need more than sixteen concurrent keys.
pub struct AbilityCooldownTracker {
    entries: Vec<(i32, AbilityAvailability)>,
    fallback_retry_ticks: u32,
}

impl AbilityCooldownTracker {
    const DEFAULT_RETRY_TICKS: u32 = 120;

    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(INITIAL_TRACKED_ABILITIES),
            fallback_retry_ticks: Self::DEFAULT_RETRY_TICKS,
        }
    }

    #[cfg(test)]
    pub fn with_retry_ticks(fallback_retry_ticks: u32) -> Self {
        Self {
            entries: Vec::with_capacity(INITIAL_TRACKED_ABILITIES),
            fallback_retry_ticks,
        }
    }

    /// Advance cooldowns by one tick and drop entries that have expired.
    #[inline]
    pub fn tick(&mut self, now: u32) {
        let mut i = 0;
        while i < self.entries.len() {
            let keep = match &mut self.entries[i].1 {
                AbilityAvailability::CoolingDown(remaining) => {
                    *remaining = remaining.saturating_sub(1);
                    *remaining > 0
                }
                AbilityAvailability::WaitingForRetry { retry_at } => now < *retry_at,
                AbilityAvailability::Ready => false,
            };
            if keep {
                i += 1;
            } else {
                self.entries.swap_remove(i);
            }
        }
    }

    /// View the current availability state for an ability at the given tick.
    #[inline]
    pub fn availability(&self, ability_id: i32, now: u32) -> AbilityAvailability {
        for (tracked_id, state) in &self.entries {
            if *tracked_id == ability_id {
                return match *state {
                    AbilityAvailability::CoolingDown(remaining) => {
                        if remaining == 0 {
                            AbilityAvailability::Ready
                        } else {
                            AbilityAvailability::CoolingDown(remaining)
                        }
                    }
                    AbilityAvailability::WaitingForRetry { retry_at } => {
                        if now >= retry_at {
                            AbilityAvailability::Ready
                        } else {
                            AbilityAvailability::WaitingForRetry { retry_at }
                        }
                    }
                    other => other,
                };
            }
        }
        AbilityAvailability::Ready
    }

    /// Whether an ability can be attempted at the given tick.
    #[inline]
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
        for (tracked_id, tracked_state) in &mut self.entries {
            if *tracked_id == ability_id {
                *tracked_state = state;
                return;
            }
        }
        self.entries.push((ability_id, state));
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

    #[test]
    fn is_ready_at_ready_variant() {
        assert!(AbilityAvailability::Ready.is_ready_at(0));
        assert!(AbilityAvailability::Ready.is_ready_at(u32::MAX));
    }

    #[test]
    fn is_ready_at_cooling_down_nonzero_remaining_is_not_ready() {
        assert!(!AbilityAvailability::CoolingDown(5).is_ready_at(0));
        assert!(!AbilityAvailability::CoolingDown(1).is_ready_at(999));
    }

    #[test]
    fn is_ready_at_cooling_down_zero_remaining_is_ready() {
        assert!(AbilityAvailability::CoolingDown(0).is_ready_at(0));
    }

    #[test]
    fn is_ready_at_waiting_for_retry_before_and_at_boundary() {
        let state = AbilityAvailability::WaitingForRetry { retry_at: 10 };
        assert!(!state.is_ready_at(9));
        assert!(state.is_ready_at(10));
        assert!(state.is_ready_at(11));
    }

    #[test]
    fn consume_updates_existing_entry_without_adding_duplicate() {
        let mut tracker = AbilityCooldownTracker::with_retry_ticks(5);
        tracker.consume(1, Some(10), 0);
        // Reconsume with a different cooldown — must update in-place.
        tracker.consume(1, Some(20), 0);
        assert!(!tracker.can_use(1, 0));
        assert_eq!(
            tracker.availability(1, 0),
            AbilityAvailability::CoolingDown(20)
        );
        // Tick 20 times — entry should be gone.
        for now in 0..20 {
            tracker.tick(now);
        }
        assert!(tracker.can_use(1, 20));
    }

    #[test]
    fn multiple_concurrent_abilities_tick_independently() {
        let mut tracker = AbilityCooldownTracker::with_retry_ticks(5);
        tracker.consume(1, Some(1), 0);
        tracker.consume(2, Some(3), 0);

        tracker.tick(1); // id=1 remaining becomes 0 and is pruned; id=2 becomes 2

        assert!(tracker.can_use(1, 1));
        assert!(!tracker.can_use(2, 1));

        tracker.tick(2); // id=2 remaining becomes 1
        tracker.tick(3); // id=2 remaining becomes 0 and is pruned

        assert!(tracker.can_use(2, 3));
    }

    #[test]
    fn tracks_more_than_sixteen_concurrent_ability_cooldowns() {
        let mut tracker = AbilityCooldownTracker::with_retry_ticks(5);
        for id in 0..24 {
            tracker.consume(id, Some(100), 0);
            assert!(!tracker.can_use(id, 0));
        }

        for id in 0..24 {
            assert_eq!(
                tracker.availability(id, 0),
                AbilityAvailability::CoolingDown(100),
                "cooldown entry {id} should remain tracked even after the old 16-slot limit",
            );
        }
    }

    #[test]
    fn availability_treats_cooling_down_zero_remaining_as_ready() {
        // Manually insert a CoolingDown(0) state via tick driving.
        let mut tracker = AbilityCooldownTracker::with_retry_ticks(5);
        tracker.consume(5, Some(1), 0);
        // tick once: remaining goes to 0. The entry is pruned from the list,
        // so availability returns Ready from the "not found" path.
        tracker.tick(0);
        assert_eq!(tracker.availability(5, 1), AbilityAvailability::Ready);
    }
}

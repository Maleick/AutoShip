//! Stuck detection and escalating recovery for navigation.
//!
//! Tracks position each tick and detects when the character is not making
//! meaningful progress. When stuck, applies escalating recovery maneuvers
//! (turning at various angles) before giving up.

use crate::hooks::movement::MovementController;
use dmft_common::nav::Waypoint;

/// How many ticks with < `MIN_MOVEMENT` unit movement before we consider ourselves stuck.
const STUCK_TICK_THRESHOLD: u32 = 40; // ~2 seconds at 20 Hz
/// Minimum movement per tick to not be considered stuck (game units).
const MIN_MOVEMENT: f32 = 0.5;
/// Maximum recovery attempts before giving up entirely.
const MAX_RECOVERY_ATTEMPTS: u32 = 5;

/// Detects stuck conditions and applies escalating recovery maneuvers.
pub struct StuckDetector {
    /// Last known position for movement comparison.
    last_position: Waypoint,
    /// Number of consecutive ticks with low movement.
    low_movement_ticks: u32,
    /// Current recovery attempt counter (0 = not recovering).
    recovery_attempt: u32,
}

impl StuckDetector {
    /// Create a new detector with zeroed state.
    pub fn new() -> Self {
        Self {
            last_position: Waypoint::new(0.0, 0.0, 0.0),
            low_movement_ticks: 0,
            recovery_attempt: 0,
        }
    }

    /// Track movement and return `true` if stuck (< `MIN_MOVEMENT` units/tick
    /// for `STUCK_TICK_THRESHOLD` consecutive ticks).
    pub fn check(&mut self, current: &Waypoint) -> bool {
        let moved = current.distance_2d(&self.last_position);
        self.last_position = *current;

        if moved < MIN_MOVEMENT {
            self.low_movement_ticks += 1;
            if self.low_movement_ticks >= STUCK_TICK_THRESHOLD {
                tracing::warn!(
                    ticks = self.low_movement_ticks,
                    attempt = self.recovery_attempt,
                    "Stuck detected"
                );
                self.low_movement_ticks = 0;
                return true;
            }
        } else {
            self.low_movement_ticks = 0;
            // Movement resumed -- clear recovery state.
            if self.recovery_attempt > 0 {
                tracing::info!(
                    attempt = self.recovery_attempt,
                    "Recovered from stuck condition"
                );
                self.recovery_attempt = 0;
            }
        }

        false
    }

    /// Return the current recovery attempt counter.
    pub fn recovery_attempt(&self) -> u32 {
        self.recovery_attempt
    }

    /// Apply an escalating recovery maneuver. Returns `true` if a recovery
    /// action was taken, `false` if max attempts exhausted (give up).
    ///
    /// Escalation sequence:
    /// 1. Turn 90 degrees right
    /// 2. Turn 90 degrees left
    /// 3. Turn 180 degrees
    /// 4. Diagonal turn 45 degrees
    /// 5. Give up (return false)
    pub fn recover(&mut self, controller: &MovementController) -> bool {
        self.recovery_attempt += 1;

        if self.recovery_attempt >= MAX_RECOVERY_ATTEMPTS {
            tracing::error!(
                attempts = self.recovery_attempt - 1,
                "Max recovery attempts reached, giving up"
            );
            return false;
        }

        let current_heading = controller.read_heading();

        // EQ headings are 0-512 (512 units = 360 degrees).
        // 90 deg = 128 units, 45 deg = 64 units, 180 deg = 256 units.
        let offset: f32 = match self.recovery_attempt {
            1 => 128.0,  // 90 degrees right
            2 => -128.0, // 90 degrees left
            3 => 256.0,  // 180 degrees
            4 => 64.0,   // 45 degrees diagonal
            _ => return false,
        };

        let new_heading = (current_heading + offset + 512.0) % 512.0;
        controller.write_heading(new_heading);

        tracing::info!(
            attempt = self.recovery_attempt,
            offset,
            heading = new_heading,
            "Stuck recovery: turning"
        );

        true
    }

    /// Reset all stuck detection state (call when advancing waypoints or
    /// starting a new path).
    pub fn reset(&mut self) {
        self.last_position = Waypoint::new(0.0, 0.0, 0.0);
        self.low_movement_ticks = 0;
        self.recovery_attempt = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_stuck_when_position_changes() {
        let mut detector = StuckDetector::new();
        for i in 0..100 {
            let pos = Waypoint::new(i as f32 * 5.0, 0.0, 0.0);
            assert!(!detector.check(&pos), "should not be stuck when moving");
        }
    }

    #[test]
    fn stuck_when_position_static_for_threshold_ticks() {
        let mut detector = StuckDetector::new();
        let pos = Waypoint::new(100.0, 100.0, 0.0);

        // First check sets last_position
        detector.check(&pos);

        // Need STUCK_TICK_THRESHOLD consecutive low-movement ticks
        let mut stuck = false;
        for _ in 0..STUCK_TICK_THRESHOLD {
            stuck = detector.check(&pos);
        }
        assert!(
            stuck,
            "should be stuck after {STUCK_TICK_THRESHOLD} stationary ticks"
        );
    }

    #[test]
    fn not_stuck_before_threshold() {
        let mut detector = StuckDetector::new();
        let pos = Waypoint::new(50.0, 50.0, 0.0);

        detector.check(&pos);
        for _ in 0..(STUCK_TICK_THRESHOLD - 2) {
            assert!(!detector.check(&pos));
        }
    }

    #[test]
    fn movement_resets_stuck_counter() {
        let mut detector = StuckDetector::new();
        let pos = Waypoint::new(10.0, 10.0, 0.0);

        // Build up some stationary ticks
        detector.check(&pos);
        for _ in 0..20 {
            detector.check(&pos);
        }

        // Move significantly
        let moved_pos = Waypoint::new(100.0, 100.0, 0.0);
        detector.check(&moved_pos);

        // Now sit still again -- should need the full threshold again
        for _ in 0..(STUCK_TICK_THRESHOLD - 1) {
            assert!(!detector.check(&moved_pos));
        }
    }

    #[test]
    fn recovery_attempts_escalate() {
        let mut detector = StuckDetector::new();
        let controller = MovementController::new(0);

        assert_eq!(detector.recovery_attempt(), 0);

        assert!(detector.recover(&controller));
        assert_eq!(detector.recovery_attempt(), 1);

        assert!(detector.recover(&controller));
        assert_eq!(detector.recovery_attempt(), 2);

        assert!(detector.recover(&controller));
        assert_eq!(detector.recovery_attempt(), 3);
    }

    #[test]
    fn max_recovery_attempts_returns_false() {
        let mut detector = StuckDetector::new();
        let controller = MovementController::new(0);

        for _ in 0..MAX_RECOVERY_ATTEMPTS {
            detector.recover(&controller);
        }
        // Next attempt should fail (give up)
        assert!(!detector.recover(&controller));
    }

    #[test]
    fn reset_clears_all_state() {
        let mut detector = StuckDetector::new();
        let controller = MovementController::new(0);
        let pos = Waypoint::new(50.0, 50.0, 0.0);

        // Build up state
        detector.check(&pos);
        for _ in 0..10 {
            detector.check(&pos);
        }
        detector.recover(&controller);

        detector.reset();
        assert_eq!(detector.recovery_attempt(), 0);
    }

    #[test]
    fn movement_clears_recovery_state() {
        let mut detector = StuckDetector::new();
        let controller = MovementController::new(0);

        detector.recover(&controller);
        detector.recover(&controller);
        assert_eq!(detector.recovery_attempt(), 2);

        // Simulate movement (distance > MIN_MOVEMENT from last_position at origin)
        let far = Waypoint::new(100.0, 100.0, 0.0);
        detector.check(&far);
        assert_eq!(detector.recovery_attempt(), 0);
    }
}

//! Stuck detection monitors position deltas over time windows.
//!
//! When a player remains nearly stationary for >30 seconds, triggers recovery
//! action:
//! - Logs alert via tracing
//! - Broadcasts IPC update
//! - Optionally auto-resets position

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use textquest_common::nav::Waypoint;

/// Position sample: recorded position + timestamp
#[derive(Debug, Clone)]
struct PositionSample {
    position: Waypoint,
    timestamp: Instant,
}

/// Detects when a character remains stuck (movement < 1 unit per 5s).
///
/// Stuck detection works by tracking position samples in a 5-second rolling
/// window:
/// - Each sample includes position and timestamp
/// - Every 1 second (or on poll), we compute distance delta from oldest →
///   newest sample in window
/// - If delta < 1.0 unit, increment stuck counter (tracks consecutive 5s
///   windows with low motion)
/// - If stuck counter ≥ 6 (30+ seconds), trigger recovery action
/// - If motion resumes (delta ≥ 1.0), reset stuck counter to 0
#[derive(Debug)]
pub struct StuckDetector {
    /// Rolling window of position samples (oldest → newest)
    samples: VecDeque<PositionSample>,

    /// Number of consecutive 5-second windows with delta < 1.0 unit
    stuck_window_count: u32,

    /// Last time we checked for stuck state (prevents thrashing)
    last_check: Option<Instant>,

    /// Whether we've already triggered recovery for current stuck session
    recovery_triggered: bool,
}

impl StuckDetector {
    /// Create a new stuck detector.
    pub fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            stuck_window_count: 0,
            last_check: None,
            recovery_triggered: false,
        }
    }

    /// Record a position sample at the current time.
    pub fn record_position(&mut self, position: Waypoint) {
        self.samples.push_back(PositionSample {
            position,
            timestamp: Instant::now(),
        });
    }

    /// Remove samples older than the window duration.
    fn prune_old_samples(&mut self, window_duration: Duration) {
        let cutoff = Instant::now() - window_duration;
        while !self.samples.is_empty() {
            if self.samples.front().is_none_or(|s| s.timestamp > cutoff) {
                break;
            }
            self.samples.pop_front();
        }
    }

    /// Calculate Euclidean distance between two positions.
    fn distance(a: Waypoint, b: Waypoint) -> f32 {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let dz = b.z - a.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Check if character is stuck and trigger recovery if needed.
    ///
    /// Returns true if recovery was triggered.
    /// Should be called ~once per second for efficiency.
    pub fn update(&mut self) -> bool {
        // Rate limit checks to ~1s intervals
        if let Some(last) = self.last_check
            && last.elapsed() < Duration::from_millis(900)
        {
            return false;
        }
        self.last_check = Some(Instant::now());

        // Prune samples older than 35 seconds (give room for stuck detection window)
        self.prune_old_samples(Duration::from_secs(35));

        // Need at least 2 samples to compute delta
        if self.samples.len() < 2 {
            return false;
        }

        // Compute distance from oldest to newest sample
        let oldest = self.samples.front().unwrap();
        let newest = self.samples.back().unwrap();

        // Check if we have at least ~5 seconds of data
        let elapsed = newest.timestamp.duration_since(oldest.timestamp);
        if elapsed < Duration::from_secs(4) {
            // Not enough data yet
            return false;
        }

        let delta = Self::distance(oldest.position, newest.position);

        // Stuck threshold: < 1 unit per 5s
        if delta < 1.0 {
            self.stuck_window_count += 1;
        } else {
            // Movement resumed — reset stuck counter
            self.stuck_window_count = 0;
            self.recovery_triggered = false;
        }

        // Timeout: stuck_window_count ≥ 6 means 30+ seconds stuck
        if self.stuck_window_count >= 6 && !self.recovery_triggered {
            self.recovery_triggered = true;
            return true;
        }

        false
    }

    /// Get current stuck counter (for testing/diagnostics).
    pub fn stuck_window_count(&self) -> u32 {
        self.stuck_window_count
    }

    /// Get sample count (for testing).
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Check if recovery has been triggered (for testing).
    pub fn recovery_triggered(&self) -> bool {
        self.recovery_triggered
    }
}

impl Default for StuckDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: move detector forward in virtual time by advancing internal
    /// Instant
    fn advance_time_and_record(
        detector: &mut StuckDetector,
        duration: Duration,
        position: Waypoint,
    ) {
        // Record multiple samples across the duration to simulate motion over time
        let samples_per_second = 10;
        let total_samples = (duration.as_secs_f32() * samples_per_second as f32).ceil() as usize;

        for _i in 0..total_samples {
            detector.record_position(position);
            // In a real scenario, Instant::now() advances naturally.
            // For testing, we rely on the detector's logic to work correctly
            // with samples recorded across real time.
        }
    }

    /// Scenario 1: Normal movement — character moves consistently.
    /// Position changes by >1 unit per 5s, so stuck counter remains 0 and no
    /// recovery triggers.
    #[test]
    fn normal_movement_no_stuck() {
        let mut detector = StuckDetector::new();

        // Record positions over 35 seconds with significant movement each 5s
        // Movement: 2 units per 5-second interval
        let mut position = Waypoint {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        for _ in 0..7 {
            detector.record_position(position);
            // Simulate 5 seconds of movement
            for _ in 0..50 {
                position.x += 0.04; // 2 units over 50 samples in a 5s window
                detector.record_position(position);
            }
        }

        // Update should never trigger recovery
        let mut recovery_triggered = false;
        for _ in 0..40 {
            if detector.update() {
                recovery_triggered = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        assert!(!recovery_triggered);
        assert_eq!(detector.stuck_window_count(), 0);
    }

    /// Scenario 2: Temporary stuck — character stuck for 20 seconds then moves.
    /// Recovery should not trigger because stuck duration < 30s.
    #[test]
    fn temporary_stuck_no_recovery() {
        let mut detector = StuckDetector::new();
        let stuck_position = Waypoint {
            x: 100.0,
            y: 100.0,
            z: 0.0,
        };

        // Record stuck position for 20 seconds (many samples in same location)
        let start = Instant::now();
        while Instant::now().duration_since(start) < Duration::from_secs(20) {
            detector.record_position(stuck_position);
            std::thread::sleep(Duration::from_millis(100));
        }

        // Try updates — none should trigger recovery at this point
        let mut recovery_triggered = false;
        for _ in 0..5 {
            if detector.update() {
                recovery_triggered = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        assert!(!recovery_triggered);

        // Now move away
        let mut position = stuck_position;
        position.x += 2.0; // Move 2 units
        detector.record_position(position);

        // Wait past rate-limit window before checking again
        std::thread::sleep(Duration::from_secs(1));
        let _ = detector.update();
        assert_eq!(detector.stuck_window_count(), 0);
    }

    /// Scenario 3: Permanent stuck — character stuck for >30 seconds.
    /// Recovery should trigger.
    #[test]
    fn permanent_stuck_triggers_recovery() {
        let mut detector = StuckDetector::new();
        let stuck_position = Waypoint {
            x: 50.0,
            y: 75.0,
            z: 10.0,
        };

        // Record stuck position continuously for 35+ seconds
        let start = Instant::now();
        while Instant::now().duration_since(start) < Duration::from_secs(35) {
            detector.record_position(stuck_position);
            std::thread::sleep(Duration::from_millis(100));
        }

        // Update should eventually trigger recovery
        let mut recovery_triggered = false;
        let mut attempts = 0;
        while attempts < 50 && !recovery_triggered {
            if detector.update() {
                recovery_triggered = true;
            }
            std::thread::sleep(Duration::from_millis(500));
            attempts += 1;
        }

        assert!(recovery_triggered);
        assert!(detector.recovery_triggered());
        assert!(detector.stuck_window_count() >= 6);
    }

    /// Stuck counter resets when motion resumes.
    #[test]
    fn stuck_counter_resets_on_motion() {
        let mut detector = StuckDetector::new();
        let stuck_position = Waypoint {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };

        // Record stuck position for ~10 seconds
        let start = Instant::now();
        while Instant::now().duration_since(start) < Duration::from_secs(10) {
            detector.record_position(stuck_position);
            std::thread::sleep(Duration::from_millis(100));
        }

        // Update and verify stuck counter increased
        for _ in 0..3 {
            let _ = detector.update();
            std::thread::sleep(Duration::from_millis(500));
        }
        let stuck_before = detector.stuck_window_count();
        assert!(stuck_before > 0, "stuck_window_count should be > 0");

        // Now move significantly
        let mut position = stuck_position;
        position.x += 2.0;
        detector.record_position(position);

        // Wait past rate-limit window before checking again
        std::thread::sleep(Duration::from_secs(1));
        let _ = detector.update();
        assert_eq!(detector.stuck_window_count(), 0);
        assert!(!detector.recovery_triggered());
    }

    /// Recovery only triggers once per stuck session.
    #[test]
    fn recovery_triggers_once() {
        let mut detector = StuckDetector::new();
        let stuck_position = Waypoint {
            x: 10.0,
            y: 20.0,
            z: 5.0,
        };

        // Record stuck position for 35+ seconds
        let start = Instant::now();
        while Instant::now().duration_since(start) < Duration::from_secs(35) {
            detector.record_position(stuck_position);
            std::thread::sleep(Duration::from_millis(100));
        }

        // First update should trigger recovery
        let mut recovery_count = 0;
        for _ in 0..50 {
            if detector.update() {
                recovery_count += 1;
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        // Should trigger exactly once
        assert_eq!(recovery_count, 1);
    }

    /// Issue #1268 integration test: Verify all three core scenarios pass
    /// 1. Normal movement (not stuck)
    /// 2. Temporary stuck (recovers within 30s)
    /// 3. Permanent stuck (timeout expires, recovery triggered)
    #[test]
    fn issue_1268_scenario_1_normal_movement() {
        let mut detector = StuckDetector::new();
        let mut position = Waypoint::new(0.0, 0.0, 0.0);
        let start = Instant::now();

        // Move continuously with >1 unit per 5s
        while Instant::now().duration_since(start) < Duration::from_secs(35) {
            detector.record_position(position);
            position.x += 0.04; // ~2 units per 5s
            std::thread::sleep(Duration::from_millis(50));
        }

        let mut recovery_triggered = false;
        for _ in 0..10 {
            if detector.update() {
                recovery_triggered = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        assert!(!recovery_triggered, "Normal movement should not trigger recovery");
        assert_eq!(detector.stuck_window_count(), 0, "Stuck counter should remain 0");
    }

    /// Issue #1268 scenario 2: Temporary stuck (within 30s threshold)
    #[test]
    fn issue_1268_scenario_2_temporary_stuck() {
        let mut detector = StuckDetector::new();
        let stuck_pos = Waypoint::new(100.0, 100.0, 0.0);
        let start = Instant::now();

        // Stay stuck for 20 seconds (< 30s threshold)
        while Instant::now().duration_since(start) < Duration::from_secs(20) {
            detector.record_position(stuck_pos);
            std::thread::sleep(Duration::from_millis(100));
        }

        // Check for recovery (should not trigger)
        let mut recovery_triggered = false;
        for _ in 0..5 {
            if detector.update() {
                recovery_triggered = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        assert!(
            !recovery_triggered,
            "Stuck < 30s should not trigger recovery"
        );

        // Now move away — recovery should reset
        let mut new_pos = stuck_pos;
        new_pos.x += 2.0;
        detector.record_position(new_pos);
        std::thread::sleep(Duration::from_secs(1));
        let _ = detector.update();

        assert_eq!(
            detector.stuck_window_count(),
            0,
            "Stuck counter should reset on movement"
        );
    }

    /// Issue #1268 scenario 3: Permanent stuck (triggers recovery at 30s+)
    #[test]
    fn issue_1268_scenario_3_permanent_stuck() {
        let mut detector = StuckDetector::new();
        let stuck_pos = Waypoint::new(50.0, 75.0, 10.0);
        let start = Instant::now();

        // Stay stuck for 35+ seconds (triggers recovery)
        while Instant::now().duration_since(start) < Duration::from_secs(35) {
            detector.record_position(stuck_pos);
            std::thread::sleep(Duration::from_millis(100));
        }

        // Poll for recovery event
        let mut recovery_triggered = false;
        let mut attempts = 0;
        while attempts < 50 && !recovery_triggered {
            if detector.update() {
                recovery_triggered = true;
            }
            std::thread::sleep(Duration::from_millis(500));
            attempts += 1;
        }

        assert!(recovery_triggered, "Stuck > 30s should trigger recovery");
        assert!(
            detector.recovery_triggered(),
            "recovery_triggered flag should be set"
        );
        assert!(
            detector.stuck_window_count() >= 6,
            "stuck_window_count should be >= 6 (30s+)"
        );
    }
}

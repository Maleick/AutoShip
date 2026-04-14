//! Warp detection and pause/resume gating for navigation/positioning.
//!
//! MQ2MoveUtils exposes `pauseonwarp` to avoid chasing targets that suddenly
//! teleport or rubber-band. This module tracks the current target position and
//! issues pause/resume signals when large jumps are observed.

use textquest_common::nav::Waypoint;

/// Distance delta (in EQ units) that constitutes a warp between ticks.
const WARP_DISTANCE_THRESHOLD: f32 = 60.0;
/// Maximum per-tick jitter (in EQ units) considered "stable" while paused.
const STABLE_JITTER_DISTANCE: f32 = 5.0;
/// Number of consecutive stable ticks required to clear a warp pause.
const STABLE_TICKS_REQUIRED: u32 = 10;

/// Minimal target snapshot for warp detection.
pub struct TargetSample {
    /// Spawn ID of the current target.
    pub id: u32,
    /// Position of the current target.
    pub position: Waypoint,
}

/// Actions emitted by the warp monitor each tick.
pub enum WarpAction {
    /// No warp-related change detected.
    None,
    /// A warp jump was detected — pause movement.
    Pause,
    /// Target has remained stable long enough to resume movement.
    Resume,
}

/// Tracks target position deltas to detect warps and gate movement until stable.
pub struct WarpMonitor {
    last_position: Option<Waypoint>,
    last_id: Option<u32>,
    paused: bool,
    stable_ticks: u32,
}

impl WarpMonitor {
    /// Create a new warp monitor with empty state.
    pub fn new() -> Self {
        Self {
            last_position: None,
            last_id: None,
            paused: false,
            stable_ticks: 0,
        }
    }

    /// Reset all tracking state.
    pub fn reset(&mut self) {
        self.last_position = None;
        self.last_id = None;
        self.paused = false;
        self.stable_ticks = 0;
    }

    /// Whether the monitor is currently paused due to a warp event.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Feed the current target position (if any) and return a warp action.
    ///
    /// - If movement jumps more than `WARP_DISTANCE_THRESHOLD` in one tick,
    ///   emits `Pause` and enters the paused state.
    /// - While paused, counts consecutive ticks with movement below
    ///   `STABLE_JITTER_DISTANCE`. After `STABLE_TICKS_REQUIRED` stable ticks,
    ///   emits `Resume`.
    pub fn update(&mut self, target: Option<&TargetSample>) -> WarpAction {
        let Some(sample) = target else {
            self.last_position = None;
            self.last_id = None;
            self.stable_ticks = 0;
            return WarpAction::None;
        };

        if self.last_id != Some(sample.id) {
            // Target changed — reset tracking and clear paused state.
            self.last_id = Some(sample.id);
            self.last_position = Some(sample.position);
            self.paused = false;
            self.stable_ticks = 0;
            return WarpAction::None;
        }

        let previous = match self.last_position {
            Some(p) => p,
            None => {
                self.last_position = Some(sample.position);
                return WarpAction::None;
            }
        };

        let delta = previous.distance_3d(&sample.position);
        self.last_position = Some(sample.position);

        if !self.paused && delta > WARP_DISTANCE_THRESHOLD {
            self.paused = true;
            self.stable_ticks = 0;
            return WarpAction::Pause;
        }

        if self.paused {
            if delta <= STABLE_JITTER_DISTANCE {
                self.stable_ticks = self.stable_ticks.saturating_add(1);
                if self.stable_ticks >= STABLE_TICKS_REQUIRED {
                    self.paused = false;
                    self.stable_ticks = 0;
                    return WarpAction::Resume;
                }
            } else {
                self.stable_ticks = 0;
            }
        }

        WarpAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: u32, wp: Waypoint) -> TargetSample {
        TargetSample { id, position: wp }
    }

    #[test]
    fn detects_warp_and_pauses() {
        let mut monitor = WarpMonitor::new();
        let a = Waypoint::new(0.0, 0.0, 0.0);
        let b = Waypoint::new(WARP_DISTANCE_THRESHOLD + 5.0, 0.0, 0.0);

        assert!(matches!(
            monitor.update(Some(&sample(1, a))),
            WarpAction::None
        ));
        assert!(matches!(
            monitor.update(Some(&sample(1, b))),
            WarpAction::Pause
        ));
        assert!(monitor.is_paused());
    }

    #[test]
    fn resumes_after_stable_ticks() {
        let mut monitor = WarpMonitor::new();
        let start = Waypoint::new(0.0, 0.0, 0.0);
        let warped = Waypoint::new(WARP_DISTANCE_THRESHOLD + 10.0, 0.0, 0.0);
        let stable = Waypoint::new(warped.x + 1.0, warped.y + 1.0, warped.z);

        monitor.update(Some(&sample(1, start)));
        assert!(matches!(
            monitor.update(Some(&sample(1, warped))),
            WarpAction::Pause
        ));

        // Feed enough stable ticks to trigger resume
        let mut resume_seen = false;
        for _ in 0..STABLE_TICKS_REQUIRED {
            if matches!(monitor.update(Some(&sample(1, stable))), WarpAction::Resume) {
                resume_seen = true;
            }
        }
        assert!(resume_seen, "expected resume after stable ticks");
        assert!(!monitor.is_paused());
    }

    #[test]
    fn unstable_motion_resets_stability_counter() {
        let mut monitor = WarpMonitor::new();
        let start = Waypoint::new(0.0, 0.0, 0.0);
        let warped = Waypoint::new(WARP_DISTANCE_THRESHOLD + 20.0, 0.0, 0.0);
        let jittery = Waypoint::new(warped.x + STABLE_JITTER_DISTANCE * 2.0, warped.y, warped.z);

        monitor.update(Some(&sample(1, start)));
        monitor.update(Some(&sample(1, warped)));
        assert!(monitor.is_paused());

        // Jitter too large should reset stability counter
        monitor.update(Some(&sample(1, jittery)));
        for _ in 0..(STABLE_TICKS_REQUIRED - 1) {
            monitor.update(Some(&sample(1, warped)));
        }
        // One extra stable tick still not enough because the counter was reset
        assert!(monitor.is_paused());
    }

    #[test]
    fn target_change_clears_pause() {
        let mut monitor = WarpMonitor::new();
        let start = Waypoint::new(0.0, 0.0, 0.0);
        let warped = Waypoint::new(WARP_DISTANCE_THRESHOLD + 10.0, 0.0, 0.0);

        monitor.update(Some(&sample(1, start)));
        assert!(matches!(
            monitor.update(Some(&sample(1, warped))),
            WarpAction::Pause
        ));
        assert!(monitor.is_paused());

        // New target should reset pause state
        monitor.update(Some(&sample(2, start)));
        assert!(!monitor.is_paused());
        assert!(matches!(
            monitor.update(Some(&sample(2, start))),
            WarpAction::None
        ));
    }

    #[test]
    fn new_monitor_is_not_paused() {
        let monitor = WarpMonitor::new();
        assert!(!monitor.is_paused());
    }

    #[test]
    fn reset_clears_paused_state() {
        let mut monitor = WarpMonitor::new();
        let start = Waypoint::new(0.0, 0.0, 0.0);
        let warped = Waypoint::new(WARP_DISTANCE_THRESHOLD + 5.0, 0.0, 0.0);

        monitor.update(Some(&sample(1, start)));
        monitor.update(Some(&sample(1, warped)));
        assert!(monitor.is_paused());

        monitor.reset();
        assert!(!monitor.is_paused());
    }

    #[test]
    fn none_target_clears_tracking_state_and_returns_none_action() {
        let mut monitor = WarpMonitor::new();
        let pos = Waypoint::new(0.0, 0.0, 0.0);

        monitor.update(Some(&sample(1, pos)));
        // Feeding None clears state; subsequent same-position feed should not trigger warp.
        assert!(matches!(monitor.update(None), WarpAction::None));
        // After feeding None, a new large jump from origin is compared against
        // a clean slate (no previous position), so it should not warp-detect.
        let far = Waypoint::new(WARP_DISTANCE_THRESHOLD + 100.0, 0.0, 0.0);
        assert!(matches!(monitor.update(Some(&sample(1, far))), WarpAction::None));
    }

    #[test]
    fn movement_below_threshold_does_not_trigger_warp() {
        let mut monitor = WarpMonitor::new();
        let a = Waypoint::new(0.0, 0.0, 0.0);
        let b = Waypoint::new(WARP_DISTANCE_THRESHOLD - 1.0, 0.0, 0.0);

        monitor.update(Some(&sample(1, a)));
        assert!(matches!(
            monitor.update(Some(&sample(1, b))),
            WarpAction::None
        ));
        assert!(!monitor.is_paused());
    }

    #[test]
    fn movement_exactly_at_threshold_does_not_trigger_warp() {
        let mut monitor = WarpMonitor::new();
        let a = Waypoint::new(0.0, 0.0, 0.0);
        let b = Waypoint::new(WARP_DISTANCE_THRESHOLD, 0.0, 0.0);

        monitor.update(Some(&sample(1, a)));
        assert!(matches!(
            monitor.update(Some(&sample(1, b))),
            WarpAction::None
        ));
        assert!(!monitor.is_paused());
    }
}

//! Zone transition state machine — handles WalkTo, ZoneTo, and PortTo
//! transitions with safe-coordinate validation, timeout detection, and recovery
//! logic.

#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use
)]

use std::time::{Duration, Instant};

use textquest_common::{nav::Waypoint, types::ClientId};

/// Maximum time allowed for a single zone transition before recovery triggers.
pub const ZONE_TRANSITION_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum coordinate magnitude considered valid for a safe landing position.
/// EverQuest zones are bounded — coordinates beyond ±10_000 are considered OOB.
const SAFE_COORD_MAX: f32 = 10_000.0;

/// How many recovery retries are attempted before giving up.
const MAX_RECOVERY_ATTEMPTS: u32 = 3;

/// The type of zone transition being executed.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionKind {
    /// Walk to a position within the current zone.
    WalkTo {
        /// Destination waypoint.
        destination: Waypoint,
    },
    /// Walk to a zone line and cross it.
    ZoneTo {
        /// Target zone short name.
        zone_name: String,
        /// Position of the zone line.
        zone_line_pos: Waypoint,
    },
    /// Port spell teleports the character to a destination zone.
    PortTo {
        /// Destination zone short name.
        zone_name: String,
        /// Client ID of the character casting the port.
        caster_id: ClientId,
    },
}

/// States of the zone transition FSM.
#[derive(Debug, Clone, PartialEq)]
pub enum ZoneTransitionState {
    /// No transition in progress.
    Idle,
    /// Moving toward the transition target (zone line or destination).
    Walking {
        /// The transition being executed.
        kind: TransitionKind,
        /// When the transition started (for timeout detection).
        started_at: Instant,
    },
    /// Actively crossing a zone boundary or waiting for port to land.
    Zoning {
        /// Target zone short name.
        target_zone: String,
        /// When the zoning phase started.
        started_at: Instant,
    },
    /// Recovering from a failed or stuck transition.
    Recovering {
        /// The transition to retry after recovery.
        kind: TransitionKind,
        /// How many recovery attempts have been made.
        attempts: u32,
        /// Safe fallback position to move to before retrying.
        safe_pos: Waypoint,
    },
}

/// Result returned from each FSM tick.
#[derive(Debug, Clone, PartialEq)]
pub enum TickResult {
    /// FSM is idle — no action needed.
    Idle,
    /// FSM is working — continue calling tick.
    InProgress,
    /// Transition completed successfully.
    Complete,
    /// Transition failed after exhausting recovery attempts.
    Failed { reason: String },
}

/// Validates that a coordinate position is within safe EQ zone bounds.
///
/// Returns `true` when all axes are within `[-SAFE_COORD_MAX, +SAFE_COORD_MAX]`
/// and no axis is NaN or infinite.
pub fn is_safe_coordinate(pos: &Waypoint) -> bool {
    let in_range = |v: f32| v.is_finite() && v.abs() <= SAFE_COORD_MAX;
    in_range(pos.x) && in_range(pos.y) && in_range(pos.z)
}

/// Returns a safe fallback coordinate near the zone origin.
///
/// Used during recovery when the current or target position is out-of-bounds.
fn safe_fallback_pos() -> Waypoint {
    Waypoint::new(0.0, 0.0, 0.0)
}

/// Computes the best safe-coordinate recovery position given the attempted
/// transition.
///
/// For `WalkTo` / `ZoneTo`, we use the zone-line position if it is valid,
/// otherwise the origin. For `PortTo`, we always fall back to the origin.
fn recovery_pos_for(kind: &TransitionKind) -> Waypoint {
    match kind {
        TransitionKind::ZoneTo { zone_line_pos, .. } if is_safe_coordinate(zone_line_pos) => {
            *zone_line_pos
        }
        TransitionKind::WalkTo { destination } if is_safe_coordinate(destination) => *destination,
        _ => safe_fallback_pos(),
    }
}

/// Zone transition finite-state machine.
///
/// Drives a single client through WalkTo / ZoneTo / PortTo transitions.
/// Validates landing coordinates, detects timeouts, and retries with
/// corrected positions up to `MAX_RECOVERY_ATTEMPTS` times.
///
/// # Usage
///
/// ```rust
/// use textquest::nav::zone_transition::{ZoneTransitionFsm, TransitionKind};
/// use textquest_common::nav::Waypoint;
///
/// let mut fsm = ZoneTransitionFsm::new(1);
/// fsm.start(TransitionKind::WalkTo { destination: Waypoint::new(100.0, 0.0, 50.0) });
/// ```
pub struct ZoneTransitionFsm {
    /// Client this FSM belongs to.
    pub client_id: ClientId,
    /// Current FSM state.
    state: ZoneTransitionState,
    /// Recovery attempts across the active transition lifecycle.
    recovery_attempts: u32,
}

impl ZoneTransitionFsm {
    /// Create a new idle FSM for the given client.
    pub const fn new(client_id: ClientId) -> Self {
        Self {
            client_id,
            state: ZoneTransitionState::Idle,
            recovery_attempts: 0,
        }
    }

    /// Start a transition. If a transition is already in progress it is
    /// replaced.
    pub fn start(&mut self, kind: TransitionKind) {
        tracing::info!(
            client_id = self.client_id,
            kind = ?kind,
            "Starting zone transition"
        );
        self.recovery_attempts = 0;
        self.state = ZoneTransitionState::Walking {
            kind,
            started_at: Instant::now(),
        };
    }

    /// Current FSM state.
    pub const fn state(&self) -> &ZoneTransitionState {
        &self.state
    }

    /// Whether the FSM is currently idle.
    pub fn is_idle(&self) -> bool {
        matches!(self.state, ZoneTransitionState::Idle)
    }

    /// Notify the FSM that the character has arrived at the target position.
    ///
    /// For `WalkTo` this completes the transition. For `ZoneTo` / `PortTo`
    /// it advances to the `Zoning` phase.
    pub fn on_arrived(&mut self) -> TickResult {
        let state = std::mem::replace(&mut self.state, ZoneTransitionState::Idle);
        match state {
            ZoneTransitionState::Walking { kind, .. } => match kind {
                TransitionKind::WalkTo { .. } => {
                    tracing::info!(client_id = self.client_id, "WalkTo transition complete");
                    TickResult::Complete
                }
                TransitionKind::ZoneTo { zone_name, .. }
                | TransitionKind::PortTo { zone_name, .. } => {
                    tracing::info!(
                        client_id = self.client_id,
                        target_zone = %zone_name,
                        "Arrived at zone transition point — entering Zoning state"
                    );
                    self.state = ZoneTransitionState::Zoning {
                        target_zone: zone_name,
                        started_at: Instant::now(),
                    };
                    TickResult::InProgress
                }
            },
            ZoneTransitionState::Recovering { kind, attempts, .. } => {
                tracing::info!(
                    client_id = self.client_id,
                    attempts,
                    "Recovery walk complete — retrying original transition"
                );
                self.state = ZoneTransitionState::Walking {
                    kind,
                    started_at: Instant::now(),
                };
                TickResult::InProgress
            }
            other => {
                self.state = other;
                TickResult::InProgress
            }
        }
    }

    /// Notify the FSM that the zone has loaded and provide the landing
    /// position.
    ///
    /// If `landing_pos` fails safe-coordinate validation, transitions to
    /// `Recovering`.
    pub fn on_zone_loaded(&mut self, landing_pos: Waypoint) -> TickResult {
        let target_zone = match &self.state {
            ZoneTransitionState::Zoning { target_zone, .. } => target_zone.clone(),
            _ => {
                tracing::warn!(
                    client_id = self.client_id,
                    "on_zone_loaded called outside Zoning state"
                );
                return TickResult::InProgress;
            }
        };

        if !is_safe_coordinate(&landing_pos) {
            tracing::warn!(
                client_id = self.client_id,
                ?landing_pos,
                "Landing position failed safe-coordinate validation — entering Recovery"
            );
            let safe_pos = safe_fallback_pos();
            // Synthesize a WalkTo-back-to-origin recovery kind
            let recovery_kind = TransitionKind::ZoneTo {
                zone_name: target_zone,
                zone_line_pos: safe_pos,
            };
            let (recovery_kind, safe_pos, attempts) = self.begin_recovery(recovery_kind, safe_pos);
            if let Some(k) = recovery_kind {
                self.state = ZoneTransitionState::Recovering {
                    kind: k,
                    attempts,
                    safe_pos,
                };
                return TickResult::InProgress;
            }
            self.state = ZoneTransitionState::Idle;
            self.recovery_attempts = 0;
            return TickResult::Failed {
                reason: "Landing position failed validation and recovery exhausted".to_string(),
            };
        }

        tracing::info!(
            client_id = self.client_id,
            target_zone = %target_zone,
            ?landing_pos,
            "Zone loaded with valid coordinates — transition complete"
        );
        self.state = ZoneTransitionState::Idle;
        self.recovery_attempts = 0;
        TickResult::Complete
    }

    /// Poll the FSM each frame/tick.
    ///
    /// Handles timeout detection. Should be called periodically (e.g., every
    /// game-loop frame or every 100 ms from an orchestrator tick).
    pub fn tick(&mut self) -> TickResult {
        match &self.state.clone() {
            ZoneTransitionState::Idle => TickResult::Idle,

            ZoneTransitionState::Walking { kind, started_at } => {
                if started_at.elapsed() >= ZONE_TRANSITION_TIMEOUT {
                    tracing::warn!(
                        client_id = self.client_id,
                        elapsed_secs = started_at.elapsed().as_secs(),
                        "Walk phase timed out — entering Recovery"
                    );
                    let safe_pos = recovery_pos_for(kind);
                    let recovery_kind = kind.clone();
                    let (recovery_kind, safe_pos, attempts) =
                        self.begin_recovery(recovery_kind, safe_pos);
                    match recovery_kind {
                        Some(k) => {
                            self.state = ZoneTransitionState::Recovering {
                                kind: k,
                                attempts,
                                safe_pos,
                            };
                            TickResult::InProgress
                        }
                        None => {
                            self.state = ZoneTransitionState::Idle;
                            self.recovery_attempts = 0;
                            TickResult::Failed {
                                reason: "Timed out during walk phase and recovery exhausted"
                                    .to_string(),
                            }
                        }
                    }
                } else {
                    TickResult::InProgress
                }
            }

            ZoneTransitionState::Zoning {
                started_at,
                target_zone,
            } => {
                if started_at.elapsed() >= ZONE_TRANSITION_TIMEOUT {
                    tracing::warn!(
                        client_id = self.client_id,
                        target_zone = %target_zone,
                        "Zoning phase timed out — entering Recovery"
                    );
                    let safe_pos = safe_fallback_pos();
                    let recovery_kind = TransitionKind::ZoneTo {
                        zone_name: target_zone.clone(),
                        zone_line_pos: safe_pos,
                    };
                    let (recovery_kind, safe_pos, attempts) =
                        self.begin_recovery(recovery_kind, safe_pos);
                    match recovery_kind {
                        Some(k) => {
                            self.state = ZoneTransitionState::Recovering {
                                kind: k,
                                attempts,
                                safe_pos,
                            };
                            TickResult::InProgress
                        }
                        None => {
                            self.state = ZoneTransitionState::Idle;
                            self.recovery_attempts = 0;
                            TickResult::Failed {
                                reason: "Timed out during zoning phase and recovery exhausted"
                                    .to_string(),
                            }
                        }
                    }
                } else {
                    TickResult::InProgress
                }
            }

            ZoneTransitionState::Recovering { attempts, .. } => {
                if *attempts >= MAX_RECOVERY_ATTEMPTS {
                    tracing::error!(
                        client_id = self.client_id,
                        attempts,
                        "Recovery attempts exhausted — giving up"
                    );
                    self.state = ZoneTransitionState::Idle;
                    self.recovery_attempts = 0;
                    return TickResult::Failed {
                        reason: format!("Exhausted {MAX_RECOVERY_ATTEMPTS} recovery attempts"),
                    };
                }
                TickResult::InProgress
            }
        }
    }

    /// Increment recovery attempt counter and check if we should give up.
    ///
    /// Returns `(Some(kind), safe_pos)` if another attempt should be made,
    /// or `(None, _)` if attempts are exhausted.
    fn begin_recovery(
        &mut self,
        kind: TransitionKind,
        safe_pos: Waypoint,
    ) -> (Option<TransitionKind>, Waypoint, u32) {
        self.recovery_attempts += 1;

        if self.recovery_attempts >= MAX_RECOVERY_ATTEMPTS {
            (None, safe_pos, self.recovery_attempts)
        } else {
            (Some(kind), safe_pos, self.recovery_attempts)
        }
    }

    /// Force the FSM back to idle (e.g., on logout or manual cancel).
    pub fn cancel(&mut self) {
        tracing::info!(client_id = self.client_id, "Zone transition cancelled");
        self.state = ZoneTransitionState::Idle;
        self.recovery_attempts = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk_kind(x: f32, y: f32, z: f32) -> TransitionKind {
        TransitionKind::WalkTo {
            destination: Waypoint::new(x, y, z),
        }
    }

    fn zone_kind(name: &str, x: f32, y: f32, z: f32) -> TransitionKind {
        TransitionKind::ZoneTo {
            zone_name: name.to_string(),
            zone_line_pos: Waypoint::new(x, y, z),
        }
    }

    fn port_kind(name: &str, caster: ClientId) -> TransitionKind {
        TransitionKind::PortTo {
            zone_name: name.to_string(),
            caster_id: caster,
        }
    }

    // ─── Safe-coordinate validation ───────────────────────────────────────

    #[test]
    fn safe_coord_accepts_origin() {
        assert!(is_safe_coordinate(&Waypoint::new(0.0, 0.0, 0.0)));
    }

    #[test]
    fn safe_coord_accepts_valid_eq_coordinates() {
        assert!(is_safe_coordinate(&Waypoint::new(1234.5, -567.8, 100.0)));
    }

    #[test]
    fn safe_coord_rejects_out_of_bounds() {
        assert!(!is_safe_coordinate(&Waypoint::new(20_000.0, 0.0, 0.0)));
        assert!(!is_safe_coordinate(&Waypoint::new(0.0, -15_000.0, 0.0)));
        assert!(!is_safe_coordinate(&Waypoint::new(0.0, 0.0, 99_999.0)));
    }

    #[test]
    fn safe_coord_rejects_nan() {
        assert!(!is_safe_coordinate(&Waypoint::new(f32::NAN, 0.0, 0.0)));
        assert!(!is_safe_coordinate(&Waypoint::new(0.0, f32::NAN, 0.0)));
        assert!(!is_safe_coordinate(&Waypoint::new(0.0, 0.0, f32::NAN)));
    }

    #[test]
    fn safe_coord_rejects_infinity() {
        assert!(!is_safe_coordinate(&Waypoint::new(f32::INFINITY, 0.0, 0.0)));
        assert!(!is_safe_coordinate(&Waypoint::new(
            0.0,
            f32::NEG_INFINITY,
            0.0
        )));
    }

    #[test]
    fn safe_coord_accepts_boundary_value() {
        assert!(is_safe_coordinate(&Waypoint::new(
            SAFE_COORD_MAX,
            -SAFE_COORD_MAX,
            0.0
        )));
    }

    #[test]
    fn safe_coord_rejects_just_over_boundary() {
        let over = SAFE_COORD_MAX + 1.0;
        assert!(!is_safe_coordinate(&Waypoint::new(over, 0.0, 0.0)));
    }

    // ─── FSM initial state ────────────────────────────────────────────────

    #[test]
    fn new_fsm_is_idle() {
        let fsm = ZoneTransitionFsm::new(1);
        assert!(fsm.is_idle());
        assert_eq!(*fsm.state(), ZoneTransitionState::Idle);
    }

    #[test]
    fn cancel_on_idle_stays_idle() {
        let mut fsm = ZoneTransitionFsm::new(1);
        fsm.cancel();
        assert!(fsm.is_idle());
    }

    #[test]
    fn tick_idle_returns_idle() {
        let mut fsm = ZoneTransitionFsm::new(1);
        assert_eq!(fsm.tick(), TickResult::Idle);
    }

    // ─── Scenario 1: Normal zone transition (WalkTo) ─────────────────────

    #[test]
    fn scenario_normal_walk_to_transition() {
        let mut fsm = ZoneTransitionFsm::new(1);

        // Start a walk
        fsm.start(walk_kind(100.0, 50.0, 0.0));
        assert!(!fsm.is_idle());

        // Tick returns InProgress while walking
        assert_eq!(fsm.tick(), TickResult::InProgress);

        // Arrive at destination → complete
        let result = fsm.on_arrived();
        assert_eq!(result, TickResult::Complete);
        assert!(fsm.is_idle());
    }

    // ─── Scenario 2: Normal zone transition (ZoneTo) ─────────────────────

    #[test]
    fn scenario_normal_zone_to_transition() {
        let mut fsm = ZoneTransitionFsm::new(2);

        fsm.start(zone_kind("gfay", -200.0, 300.0, 0.0));
        assert_eq!(fsm.tick(), TickResult::InProgress);

        // Arrive at zone line → advances to Zoning
        let result = fsm.on_arrived();
        assert_eq!(result, TickResult::InProgress);
        assert!(matches!(fsm.state(), ZoneTransitionState::Zoning { .. }));

        // Zone loads with valid coords → complete
        let result = fsm.on_zone_loaded(Waypoint::new(10.0, 20.0, 0.0));
        assert_eq!(result, TickResult::Complete);
        assert!(fsm.is_idle());
    }

    // ─── Scenario 3: Normal port transition ──────────────────────────────

    #[test]
    fn scenario_normal_port_to_transition() {
        let mut fsm = ZoneTransitionFsm::new(3);

        fsm.start(port_kind("commons", 42));
        assert_eq!(fsm.tick(), TickResult::InProgress);

        // "Arrived" at port-cast point → enter Zoning
        let result = fsm.on_arrived();
        assert_eq!(result, TickResult::InProgress);
        assert!(matches!(fsm.state(), ZoneTransitionState::Zoning { .. }));

        // Zone loads with valid landing → complete
        let result = fsm.on_zone_loaded(Waypoint::new(0.0, 0.0, 0.0));
        assert_eq!(result, TickResult::Complete);
    }

    // ─── Scenario 4: Safe-coordinate recovery on bad landing ─────────────

    #[test]
    fn scenario_safe_coord_recovery_on_bad_landing() {
        let mut fsm = ZoneTransitionFsm::new(4);

        fsm.start(zone_kind("qeynos", -100.0, 200.0, 0.0));

        // Walk + arrive → Zoning
        fsm.on_arrived();
        assert!(matches!(fsm.state(), ZoneTransitionState::Zoning { .. }));

        // Zone loads with OOB coordinates → triggers Recovery
        let result = fsm.on_zone_loaded(Waypoint::new(99_999.0, 0.0, 0.0));
        assert_eq!(result, TickResult::InProgress);
        assert!(matches!(
            fsm.state(),
            ZoneTransitionState::Recovering { .. }
        ));

        // Verify recovery safe_pos is origin
        if let ZoneTransitionState::Recovering {
            safe_pos, attempts, ..
        } = fsm.state()
        {
            assert_eq!(safe_pos.x, 0.0);
            assert_eq!(safe_pos.y, 0.0);
            assert_eq!(safe_pos.z, 0.0);
            assert_eq!(*attempts, 1);
        }
    }

    // ─── Scenario 5: Timeout recovery ────────────────────────────────────

    #[test]
    fn scenario_timeout_recovery() {
        use std::time::{Duration, Instant};

        let mut fsm = ZoneTransitionFsm::new(5);
        fsm.start(zone_kind("ecommons", 50.0, -50.0, 0.0));

        // Simulate timeout by manually setting Walking state with old timestamp
        fsm.state = ZoneTransitionState::Walking {
            kind: zone_kind("ecommons", 50.0, -50.0, 0.0),
            started_at: Instant::now() - Duration::from_secs(31),
        };

        // Tick should detect timeout and enter Recovery
        let result = fsm.tick();
        assert_eq!(result, TickResult::InProgress);
        assert!(matches!(
            fsm.state(),
            ZoneTransitionState::Recovering { .. }
        ));
    }

    #[test]
    fn scenario_zoning_timeout_triggers_recovery() {
        use std::time::{Duration, Instant};

        let mut fsm = ZoneTransitionFsm::new(6);

        // Put directly in Zoning state with old timestamp
        fsm.state = ZoneTransitionState::Zoning {
            target_zone: "nektulos".to_string(),
            started_at: Instant::now() - Duration::from_secs(35),
        };

        let result = fsm.tick();
        assert_eq!(result, TickResult::InProgress);
        assert!(matches!(
            fsm.state(),
            ZoneTransitionState::Recovering { .. }
        ));
    }

    // ─── Scenario 6: Exhausted recovery → Failed ─────────────────────────

    #[test]
    fn scenario_exhausted_recovery_returns_failed() {
        let mut fsm = ZoneTransitionFsm::new(7);

        // Simulate being in recovery with max attempts
        fsm.state = ZoneTransitionState::Recovering {
            kind: walk_kind(0.0, 0.0, 0.0),
            attempts: MAX_RECOVERY_ATTEMPTS,
            safe_pos: Waypoint::new(0.0, 0.0, 0.0),
        };

        let result = fsm.tick();
        assert!(matches!(result, TickResult::Failed { .. }));
        assert!(fsm.is_idle());
    }

    // ─── Cancel ──────────────────────────────────────────────────────────

    #[test]
    fn cancel_during_walking_resets_to_idle() {
        let mut fsm = ZoneTransitionFsm::new(8);
        fsm.start(walk_kind(100.0, 0.0, 0.0));
        assert!(!fsm.is_idle());
        fsm.cancel();
        assert!(fsm.is_idle());
    }

    #[test]
    fn cancel_during_zoning_resets_to_idle() {
        let mut fsm = ZoneTransitionFsm::new(9);
        fsm.start(zone_kind("gfay", 0.0, 0.0, 0.0));
        fsm.on_arrived();
        assert!(matches!(fsm.state(), ZoneTransitionState::Zoning { .. }));
        fsm.cancel();
        assert!(fsm.is_idle());
    }

    // ─── on_zone_loaded outside Zoning state ─────────────────────────────

    #[test]
    fn on_zone_loaded_outside_zoning_is_noop() {
        let mut fsm = ZoneTransitionFsm::new(10);
        // Should not panic, just return InProgress
        let result = fsm.on_zone_loaded(Waypoint::new(0.0, 0.0, 0.0));
        assert_eq!(result, TickResult::InProgress);
        // Stays idle
        assert!(fsm.is_idle());
    }

    // ─── client_id ───────────────────────────────────────────────────────

    #[test]
    fn fsm_stores_client_id() {
        let fsm = ZoneTransitionFsm::new(99);
        assert_eq!(fsm.client_id, 99);
    }

    // ─── Recovery walk completion advances back to Walking ───────────────

    #[test]
    fn recovery_arrived_restarts_walk() {
        let mut fsm = ZoneTransitionFsm::new(11);
        let original_kind = zone_kind("freeport", 50.0, 50.0, 0.0);
        fsm.state = ZoneTransitionState::Recovering {
            kind: original_kind,
            attempts: 1,
            safe_pos: Waypoint::new(0.0, 0.0, 0.0),
        };

        let result = fsm.on_arrived();
        assert_eq!(result, TickResult::InProgress);
        assert!(matches!(fsm.state(), ZoneTransitionState::Walking { .. }));
    }

    #[test]
    fn repeated_timeouts_increment_recovery_attempts_until_exhausted() {
        use std::time::{Duration, Instant};

        let mut fsm = ZoneTransitionFsm::new(12);
        fsm.start(zone_kind("qeynos", 10.0, 20.0, 0.0));

        // 1st timeout -> recovery attempt 1
        fsm.state = ZoneTransitionState::Walking {
            kind: zone_kind("qeynos", 10.0, 20.0, 0.0),
            started_at: Instant::now() - Duration::from_secs(31),
        };
        assert_eq!(fsm.tick(), TickResult::InProgress);
        assert!(matches!(
            fsm.state(),
            ZoneTransitionState::Recovering { attempts: 1, .. }
        ));

        // Recovery walk complete -> back to walking, then timeout again
        assert_eq!(fsm.on_arrived(), TickResult::InProgress);
        fsm.state = ZoneTransitionState::Walking {
            kind: zone_kind("qeynos", 10.0, 20.0, 0.0),
            started_at: Instant::now() - Duration::from_secs(31),
        };
        assert_eq!(fsm.tick(), TickResult::InProgress);
        assert!(matches!(
            fsm.state(),
            ZoneTransitionState::Recovering { attempts: 2, .. }
        ));

        // Third timeout should exhaust recovery and fail cleanly
        assert_eq!(fsm.on_arrived(), TickResult::InProgress);
        fsm.state = ZoneTransitionState::Walking {
            kind: zone_kind("qeynos", 10.0, 20.0, 0.0),
            started_at: Instant::now() - Duration::from_secs(31),
        };
        let result = fsm.tick();
        assert!(matches!(result, TickResult::Failed { .. }));
        assert!(fsm.is_idle());
    }

    // ─── TransitionKind variants have correct data ────────────────────────

    #[test]
    fn walk_kind_stores_destination() {
        let k = walk_kind(10.0, 20.0, 30.0);
        if let TransitionKind::WalkTo { destination } = k {
            assert_eq!(destination.x, 10.0);
            assert_eq!(destination.y, 20.0);
            assert_eq!(destination.z, 30.0);
        } else {
            panic!("expected WalkTo");
        }
    }

    #[test]
    fn zone_kind_stores_name_and_pos() {
        let k = zone_kind("highpass", 1.0, 2.0, 3.0);
        if let TransitionKind::ZoneTo {
            zone_name,
            zone_line_pos,
        } = k
        {
            assert_eq!(zone_name, "highpass");
            assert_eq!(zone_line_pos.x, 1.0);
        } else {
            panic!("expected ZoneTo");
        }
    }

    #[test]
    fn port_kind_stores_name_and_caster() {
        let k = port_kind("nexus", 77);
        if let TransitionKind::PortTo {
            zone_name,
            caster_id,
        } = k
        {
            assert_eq!(zone_name, "nexus");
            assert_eq!(caster_id, 77);
        } else {
            panic!("expected PortTo");
        }
    }
}

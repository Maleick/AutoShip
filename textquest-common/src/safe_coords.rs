//! Safe coordinate types for zone transition recovery and position validation.
//!
//! Defines coordinate validation, recovery requests/responses, and position
//! semantics used during zone transitions when landing positions are invalid
//! or out-of-bounds.

use crate::nav::Waypoint;
use serde::{Deserialize, Serialize};

/// Maximum coordinate magnitude considered valid for a safe landing position.
/// EverQuest zones are bounded — coordinates beyond ±10,000 are considered OOB.
pub const SAFE_COORD_MAX: f32 = 10_000.0;

/// Minimum coordinate difference to consider a position "moved" for validation.
/// Avoids false positives from floating-point noise.
pub const POSITION_EPSILON: f32 = 0.1;

/// Request sent from orchestrator to DLL to get safe coordinates after zone
/// denial.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafeCoordRequest {
    /// Zone short name (e.g., "qey2hh1").
    pub zone: String,

    /// Current invalid position that triggered recovery.
    pub invalid_pos: (f32, f32, f32),

    /// Preferred fallback position (if known from navmesh).
    pub fallback_pos: Option<(f32, f32, f32)>,
}

impl SafeCoordRequest {
    /// Create a new safe coordinate recovery request.
    pub fn new(zone: String, invalid_pos: (f32, f32, f32)) -> Self {
        Self {
            zone,
            invalid_pos,
            fallback_pos: None,
        }
    }

    /// Set a preferred fallback position.
    pub fn with_fallback(mut self, fallback: (f32, f32, f32)) -> Self {
        self.fallback_pos = Some(fallback);
        self
    }
}

/// Response from DLL containing validated safe coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafeCoordResponse {
    /// The safe coordinate validated by the DLL (in-zone, within bounds).
    pub safe_pos: (f32, f32, f32),

    /// Whether this position was pre-calculated (e.g., zone origin) or looked
    /// up from navmesh/zone data.
    pub is_validated: bool,

    /// Human-readable reason for the selected position (e.g., "zone origin",
    /// "nearest navmesh point", "fallback provided").
    pub reason: String,
}

impl SafeCoordResponse {
    /// Create a response with origin (0, 0, 0) as safe position.
    pub fn origin(reason: impl Into<String>) -> Self {
        Self {
            safe_pos: (0.0, 0.0, 0.0),
            is_validated: true,
            reason: reason.into(),
        }
    }

    /// Create a response with a custom safe position.
    pub fn custom(pos: (f32, f32, f32), is_validated: bool, reason: impl Into<String>) -> Self {
        Self {
            safe_pos: pos,
            is_validated,
            reason: reason.into(),
        }
    }
}

/// Validates that a coordinate position is within safe EQ zone bounds.
///
/// Returns `true` when all axes are within `[-SAFE_COORD_MAX, +SAFE_COORD_MAX]`
/// and no axis is NaN or infinite.
///
/// # Examples
///
/// ```
/// use textquest_common::safe_coords::is_safe_coordinate;
/// use textquest_common::nav::Waypoint;
///
/// assert!(is_safe_coordinate(&Waypoint::new(0.0, 0.0, 0.0)));
/// assert!(is_safe_coordinate(&Waypoint::new(1000.0, -500.0, 100.0)));
/// assert!(!is_safe_coordinate(&Waypoint::new(20_000.0, 0.0, 0.0))); // Out of bounds
/// assert!(!is_safe_coordinate(&Waypoint::new(f32::NAN, 0.0, 0.0))); // NaN
/// ```
pub fn is_safe_coordinate(pos: &Waypoint) -> bool {
    let in_range = |v: f32| v.is_finite() && v.abs() <= SAFE_COORD_MAX;
    in_range(pos.x) && in_range(pos.y) && in_range(pos.z)
}

/// Validates that a position (as tuple) is within safe EQ zone bounds.
///
/// Convenience function for use with raw coordinate tuples.
pub fn is_safe_coordinate_tuple(pos: (f32, f32, f32)) -> bool {
    let in_range = |v: f32| v.is_finite() && v.abs() <= SAFE_COORD_MAX;
    in_range(pos.0) && in_range(pos.1) && in_range(pos.2)
}

/// Result of a position validation check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionValidation {
    /// Position is valid and safe.
    Valid,

    /// Position is out of bounds (exceeds coordinate limits).
    OutOfBounds,

    /// Position is NaN or infinite (malformed).
    Invalid,

    /// Position could not be validated due to other reasons.
    Unknown,
}

impl PositionValidation {
    /// Validate a waypoint and return detailed validation result.
    pub fn check(pos: &Waypoint) -> Self {
        if !pos.x.is_finite() || !pos.y.is_finite() || !pos.z.is_finite() {
            return Self::Invalid;
        }

        if pos.x.abs() > SAFE_COORD_MAX
            || pos.y.abs() > SAFE_COORD_MAX
            || pos.z.abs() > SAFE_COORD_MAX
        {
            return Self::OutOfBounds;
        }

        Self::Valid
    }

    /// Validate a tuple coordinate.
    pub fn check_tuple(pos: (f32, f32, f32)) -> Self {
        if !pos.0.is_finite() || !pos.1.is_finite() || !pos.2.is_finite() {
            return Self::Invalid;
        }

        if pos.0.abs() > SAFE_COORD_MAX
            || pos.1.abs() > SAFE_COORD_MAX
            || pos.2.abs() > SAFE_COORD_MAX
        {
            return Self::OutOfBounds;
        }

        Self::Valid
    }

    /// Returns `true` if validation passed.
    pub fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Human-readable description of the validation result.
    pub fn description(self) -> &'static str {
        match self {
            Self::Valid => "Position is valid and within safe bounds",
            Self::OutOfBounds => "Position exceeds safe coordinate limits",
            Self::Invalid => "Position is NaN or infinite",
            Self::Unknown => "Position validation result unknown",
        }
    }
}

/// Determines if position has moved significantly enough to consider it
/// different.
///
/// Uses `POSITION_EPSILON` to avoid floating-point noise.
pub fn positions_differ(from: (f32, f32, f32), to: (f32, f32, f32)) -> bool {
    (from.0 - to.0).abs() > POSITION_EPSILON
        || (from.1 - to.1).abs() > POSITION_EPSILON
        || (from.2 - to.2).abs() > POSITION_EPSILON
}

/// Computes distance between two positions (Manhattan distance, fast
/// approximation).
pub fn position_distance_manhattan(from: (f32, f32, f32), to: (f32, f32, f32)) -> f32 {
    (from.0 - to.0).abs() + (from.1 - to.1).abs() + (from.2 - to.2).abs()
}

/// Computes distance between two positions (Euclidean distance).
pub fn position_distance_euclidean(from: (f32, f32, f32), to: (f32, f32, f32)) -> f32 {
    let dx = from.0 - to.0;
    let dy = from.1 - to.1;
    let dz = from.2 - to.2;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Safe coordinate validation tests ─────────────────────────────────

    #[test]
    fn is_safe_coordinate_at_origin() {
        assert!(is_safe_coordinate(&Waypoint::new(0.0, 0.0, 0.0)));
    }

    #[test]
    fn is_safe_coordinate_accepts_typical_eq_coords() {
        assert!(is_safe_coordinate(&Waypoint::new(100.0, -200.0, 50.0)));
        assert!(is_safe_coordinate(&Waypoint::new(1234.5, 567.8, 100.2)));
        assert!(is_safe_coordinate(&Waypoint::new(-9999.0, -5000.0, 3000.0)));
    }

    #[test]
    fn is_safe_coordinate_at_boundary() {
        assert!(is_safe_coordinate(&Waypoint::new(SAFE_COORD_MAX, 0.0, 0.0)));
        assert!(is_safe_coordinate(&Waypoint::new(
            -SAFE_COORD_MAX,
            0.0,
            0.0
        )));
        assert!(is_safe_coordinate(&Waypoint::new(0.0, SAFE_COORD_MAX, 0.0)));
        assert!(is_safe_coordinate(&Waypoint::new(
            0.0,
            -SAFE_COORD_MAX,
            0.0
        )));
    }

    #[test]
    fn is_safe_coordinate_rejects_out_of_bounds() {
        assert!(!is_safe_coordinate(&Waypoint::new(
            SAFE_COORD_MAX + 1.0,
            0.0,
            0.0
        )));
        assert!(!is_safe_coordinate(&Waypoint::new(
            -SAFE_COORD_MAX - 1.0,
            0.0,
            0.0
        )));
        assert!(!is_safe_coordinate(&Waypoint::new(0.0, 20_000.0, 0.0)));
        assert!(!is_safe_coordinate(&Waypoint::new(0.0, 0.0, 99_999.0)));
    }

    #[test]
    fn is_safe_coordinate_rejects_nan() {
        assert!(!is_safe_coordinate(&Waypoint::new(f32::NAN, 0.0, 0.0)));
        assert!(!is_safe_coordinate(&Waypoint::new(0.0, f32::NAN, 0.0)));
        assert!(!is_safe_coordinate(&Waypoint::new(0.0, 0.0, f32::NAN)));
    }

    #[test]
    fn is_safe_coordinate_rejects_infinity() {
        assert!(!is_safe_coordinate(&Waypoint::new(f32::INFINITY, 0.0, 0.0)));
        assert!(!is_safe_coordinate(&Waypoint::new(
            f32::NEG_INFINITY,
            0.0,
            0.0
        )));
        assert!(!is_safe_coordinate(&Waypoint::new(0.0, f32::INFINITY, 0.0)));
    }

    // ─── Tuple coordinate validation ──────────────────────────────────────

    #[test]
    fn is_safe_coordinate_tuple_at_origin() {
        assert!(is_safe_coordinate_tuple((0.0, 0.0, 0.0)));
    }

    #[test]
    fn is_safe_coordinate_tuple_valid() {
        assert!(is_safe_coordinate_tuple((100.0, -200.0, 50.0)));
    }

    #[test]
    fn is_safe_coordinate_tuple_rejects_oob() {
        assert!(!is_safe_coordinate_tuple((20_000.0, 0.0, 0.0)));
    }

    #[test]
    fn is_safe_coordinate_tuple_rejects_nan() {
        assert!(!is_safe_coordinate_tuple((f32::NAN, 0.0, 0.0)));
    }

    // ─── PositionValidation enum tests ───────────────────────────────────

    #[test]
    fn position_validation_check_valid() {
        let result = PositionValidation::check(&Waypoint::new(100.0, 200.0, 50.0));
        assert_eq!(result, PositionValidation::Valid);
    }

    #[test]
    fn position_validation_check_out_of_bounds() {
        let result = PositionValidation::check(&Waypoint::new(20_000.0, 0.0, 0.0));
        assert_eq!(result, PositionValidation::OutOfBounds);
    }

    #[test]
    fn position_validation_check_invalid() {
        let result = PositionValidation::check(&Waypoint::new(f32::NAN, 0.0, 0.0));
        assert_eq!(result, PositionValidation::Invalid);
    }

    #[test]
    fn position_validation_check_tuple_valid() {
        let result = PositionValidation::check_tuple((100.0, 200.0, 50.0));
        assert_eq!(result, PositionValidation::Valid);
    }

    #[test]
    fn position_validation_is_valid_true() {
        assert!(PositionValidation::Valid.is_valid());
    }

    #[test]
    fn position_validation_is_valid_false() {
        assert!(!PositionValidation::OutOfBounds.is_valid());
        assert!(!PositionValidation::Invalid.is_valid());
        assert!(!PositionValidation::Unknown.is_valid());
    }

    #[test]
    fn position_validation_description_not_empty() {
        let variants = [
            PositionValidation::Valid,
            PositionValidation::OutOfBounds,
            PositionValidation::Invalid,
            PositionValidation::Unknown,
        ];
        for v in &variants {
            assert!(!v.description().is_empty());
        }
    }

    // ─── Position difference detection ───────────────────────────────────

    #[test]
    fn positions_differ_no_change() {
        let pos = (100.0, 200.0, 50.0);
        assert!(!positions_differ(pos, pos));
    }

    #[test]
    fn positions_differ_significant_x() {
        assert!(positions_differ((100.0, 0.0, 0.0), (200.0, 0.0, 0.0)));
    }

    #[test]
    fn positions_differ_significant_y() {
        assert!(positions_differ((0.0, 100.0, 0.0), (0.0, 200.0, 0.0)));
    }

    #[test]
    fn positions_differ_significant_z() {
        assert!(positions_differ((0.0, 0.0, 100.0), (0.0, 0.0, 200.0)));
    }

    #[test]
    fn positions_differ_tiny_change_ignored() {
        let epsilon_half = POSITION_EPSILON / 2.0;
        assert!(!positions_differ((0.0, 0.0, 0.0), (epsilon_half, 0.0, 0.0)));
    }

    #[test]
    fn positions_differ_just_over_epsilon() {
        let just_over = POSITION_EPSILON + 0.001;
        assert!(positions_differ((0.0, 0.0, 0.0), (just_over, 0.0, 0.0)));
    }

    // ─── Distance calculations ────────────────────────────────────────────

    #[test]
    fn position_distance_manhattan_zero() {
        let dist = position_distance_manhattan((0.0, 0.0, 0.0), (0.0, 0.0, 0.0));
        assert!(dist.abs() < f32::EPSILON);
    }

    #[test]
    fn position_distance_manhattan_simple() {
        let dist = position_distance_manhattan((0.0, 0.0, 0.0), (3.0, 4.0, 0.0));
        assert!((dist - 7.0).abs() < f32::EPSILON);
    }

    #[test]
    fn position_distance_manhattan_with_z() {
        let dist = position_distance_manhattan((0.0, 0.0, 0.0), (1.0, 2.0, 3.0));
        assert!((dist - 6.0).abs() < f32::EPSILON);
    }

    #[test]
    fn position_distance_euclidean_zero() {
        let dist = position_distance_euclidean((0.0, 0.0, 0.0), (0.0, 0.0, 0.0));
        assert!(dist.abs() < f32::EPSILON);
    }

    #[test]
    fn position_distance_euclidean_3_4_5_triangle() {
        let dist = position_distance_euclidean((0.0, 0.0, 0.0), (3.0, 4.0, 0.0));
        assert!((dist - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn position_distance_euclidean_with_z() {
        let dist = position_distance_euclidean((0.0, 0.0, 0.0), (1.0, 1.0, 1.0));
        let expected = f32::sqrt(3.0);
        assert!((dist - expected).abs() < 0.001);
    }

    // ─── SafeCoordRequest tests ──────────────────────────────────────────

    #[test]
    fn safe_coord_request_new() {
        let req = SafeCoordRequest::new("qey2hh1".to_string(), (100.0, 200.0, 50.0));
        assert_eq!(req.zone, "qey2hh1");
        assert_eq!(req.invalid_pos, (100.0, 200.0, 50.0));
        assert!(req.fallback_pos.is_none());
    }

    #[test]
    fn safe_coord_request_with_fallback() {
        let req = SafeCoordRequest::new("qey2hh1".to_string(), (100.0, 200.0, 50.0))
            .with_fallback((10.0, 20.0, 5.0));
        assert_eq!(req.fallback_pos, Some((10.0, 20.0, 5.0)));
    }

    #[test]
    fn safe_coord_request_serialization() {
        let req = SafeCoordRequest::new("gfay".to_string(), (0.0, 0.0, 0.0));
        let json = serde_json::to_string(&req).expect("serialize");
        let restored: SafeCoordRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(req, restored);
    }

    // ─── SafeCoordResponse tests ─────────────────────────────────────────

    #[test]
    fn safe_coord_response_origin() {
        let resp = SafeCoordResponse::origin("zone origin");
        assert_eq!(resp.safe_pos, (0.0, 0.0, 0.0));
        assert!(resp.is_validated);
        assert_eq!(resp.reason, "zone origin");
    }

    #[test]
    fn safe_coord_response_custom() {
        let resp = SafeCoordResponse::custom((100.0, 200.0, 50.0), true, "navmesh point");
        assert_eq!(resp.safe_pos, (100.0, 200.0, 50.0));
        assert!(resp.is_validated);
        assert_eq!(resp.reason, "navmesh point");
    }

    #[test]
    fn safe_coord_response_unvalidated() {
        let resp = SafeCoordResponse::custom((50.0, 50.0, 50.0), false, "fallback");
        assert!(!resp.is_validated);
    }

    #[test]
    fn safe_coord_response_serialization() {
        let resp = SafeCoordResponse::origin("test");
        let json = serde_json::to_string(&resp).expect("serialize");
        let restored: SafeCoordResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(resp, restored);
    }

    // ─── Integration tests ──────────────────────────────────────────────

    #[test]
    fn recovery_flow_typical_scenario() {
        // 1. Zone denies entry with invalid landing coords
        let invalid_pos = (99_999.0, 0.0, 0.0); // OOB
        assert!(!is_safe_coordinate_tuple(invalid_pos));

        // 2. Request safe coords from DLL
        let req = SafeCoordRequest::new("qey2hh1".to_string(), invalid_pos);
        assert_eq!(req.zone, "qey2hh1");

        // 3. DLL responds with safe origin
        let resp = SafeCoordResponse::origin("zone origin");
        assert!(is_safe_coordinate_tuple(resp.safe_pos));

        // 4. Orchestrator moves player to safe pos and retries
        assert!(positions_differ(invalid_pos, resp.safe_pos));
    }

    #[test]
    fn position_validation_matches_is_safe_coordinate() {
        let positions = [
            Waypoint::new(0.0, 0.0, 0.0),
            Waypoint::new(1000.0, -500.0, 100.0),
            Waypoint::new(SAFE_COORD_MAX, 0.0, 0.0),
        ];

        for pos in &positions {
            let is_safe = is_safe_coordinate(pos);
            let validation = PositionValidation::check(pos);
            assert_eq!(is_safe, validation.is_valid());
        }
    }
}

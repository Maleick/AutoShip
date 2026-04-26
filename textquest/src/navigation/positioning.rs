//! Camp positioning utilities — distance helpers and arrival detection.
//!
//! These helpers are consumed by [`super::camp_spot::CampReturnMachine`] callers
//! to decide when a character has reached its camp spot and should call
//! `notify_arrived`.

/// 2-D position in EverQuest coordinates (X = east/west, Y = north/south).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    /// East/west coordinate.
    pub x: f32,
    /// North/south coordinate.
    pub y: f32,
}

impl Position {
    /// Construct a new position.
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to another position (2-D, ignoring Z).
    #[must_use]
    pub fn distance_to(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Returns `true` if this position is within `radius` units of `target`.
    #[must_use]
    pub fn within_radius(self, target: Self, radius: f32) -> bool {
        self.distance_to(target) <= radius
    }
}

/// Default arrival radius used by the camp return machine (units).
///
/// Characters within this radius of their assigned camp spot are considered
/// "at camp" for the purpose of the return-state machine.
pub const DEFAULT_ARRIVAL_RADIUS: f32 = 10.0;

/// Checks whether `current` is close enough to `camp_spot` to be considered
/// arrived, using [`DEFAULT_ARRIVAL_RADIUS`].
#[must_use]
pub fn has_arrived_at_camp(current: Position, camp_spot: Position) -> bool {
    current.within_radius(camp_spot, DEFAULT_ARRIVAL_RADIUS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_to_self_is_zero() {
        let p = Position::new(100.0, 200.0);
        assert!((p.distance_to(p) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn distance_3_4_5_triangle() {
        let a = Position::new(0.0, 0.0);
        let b = Position::new(3.0, 4.0);
        assert!((a.distance_to(b) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn within_radius_at_exact_distance() {
        let a = Position::new(0.0, 0.0);
        let b = Position::new(10.0, 0.0);
        assert!(a.within_radius(b, 10.0));
    }

    #[test]
    fn within_radius_outside() {
        let a = Position::new(0.0, 0.0);
        let b = Position::new(10.1, 0.0);
        assert!(!a.within_radius(b, 10.0));
    }

    #[test]
    fn has_arrived_at_camp_within_default_radius() {
        let camp = Position::new(500.0, 300.0);
        let near = Position::new(508.0, 300.0); // 8 units away
        assert!(has_arrived_at_camp(near, camp));
    }

    #[test]
    fn has_arrived_at_camp_outside_default_radius() {
        let camp = Position::new(500.0, 300.0);
        let far = Position::new(515.0, 300.0); // 15 units away
        assert!(!has_arrived_at_camp(far, camp));
    }
}

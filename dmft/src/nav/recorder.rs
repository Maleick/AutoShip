//! Waypoint recorder — captures a character's movement into a replayable path.
//! Records position snapshots at regular intervals while the character moves.

use dmft_common::nav::Waypoint;
use std::time::Instant;

/// Minimum distance between recorded waypoints to avoid redundant points.
const MIN_WAYPOINT_DISTANCE: f32 = 10.0;

/// Records a character's movement into a sequence of waypoints.
pub struct WaypointRecorder {
    waypoints: Vec<Waypoint>,
    last_position: Option<Waypoint>,
    start_time: Option<Instant>,
}

impl WaypointRecorder {
    pub fn new() -> Self {
        Self {
            waypoints: Vec::new(),
            last_position: None,
            start_time: None,
        }
    }

    /// Start recording.
    pub fn start(&mut self) {
        self.waypoints.clear();
        self.last_position = None;
        self.start_time = Some(Instant::now());
        tracing::info!("Waypoint recording started");
    }

    /// Stop recording and return the recorded path.
    pub fn stop(&mut self) -> Vec<Waypoint> {
        let elapsed = self
            .start_time
            .take()
            .map(|s| s.elapsed())
            .unwrap_or_default();
        tracing::info!(
            waypoints = self.waypoints.len(),
            elapsed_secs = elapsed.as_secs(),
            "Waypoint recording stopped"
        );
        std::mem::take(&mut self.waypoints)
    }

    /// Feed a position update from game state. Call this each time
    /// the orchestrator reads a new GameState for the recorded character.
    pub fn record_position(&mut self, x: f32, y: f32, z: f32) {
        if self.start_time.is_none() {
            return;
        }

        let current = Waypoint::new(x, y, z);

        let dominated = match self.last_position {
            Some(ref last) => current.distance_2d(last) < MIN_WAYPOINT_DISTANCE,
            None => false,
        };

        if !dominated {
            self.last_position = Some(current);
            self.waypoints.push(current);
        }
    }

    /// Whether currently recording.
    pub fn is_recording(&self) -> bool {
        self.start_time.is_some()
    }

    /// Number of waypoints recorded so far.
    pub fn waypoint_count(&self) -> usize {
        self.waypoints.len()
    }
}

/// Simplify a recorded path by removing redundant collinear points.
/// Uses the Ramer-Douglas-Peucker algorithm in 2D.
pub fn simplify_path(waypoints: &[Waypoint], epsilon: f32) -> Vec<Waypoint> {
    if waypoints.len() <= 2 {
        return waypoints.to_vec();
    }

    // Find the point with the maximum distance from the line (first, last).
    let first = &waypoints[0];
    let last = &waypoints[waypoints.len() - 1];
    let mut max_dist = 0.0f32;
    let mut max_index = 0;

    for (i, wp) in waypoints
        .iter()
        .enumerate()
        .skip(1)
        .take(waypoints.len() - 2)
    {
        let dist = point_line_distance_2d(wp, first, last);
        if dist > max_dist {
            max_dist = dist;
            max_index = i;
        }
    }

    if max_dist > epsilon {
        // Recursively simplify both halves.
        let mut left = simplify_path(&waypoints[..=max_index], epsilon);
        let right = simplify_path(&waypoints[max_index..], epsilon);
        // Remove duplicate point at the junction.
        left.pop();
        left.extend(right);
        left
    } else {
        // All intermediate points are close to the line — keep only endpoints.
        vec![*first, *last]
    }
}

/// Perpendicular distance from point to line (2D, XY plane).
fn point_line_distance_2d(point: &Waypoint, line_start: &Waypoint, line_end: &Waypoint) -> f32 {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let len_sq = dx * dx + dy * dy;

    if len_sq < f32::EPSILON {
        // Line start and end are the same point.
        return point.distance_2d(line_start);
    }

    let cross = (point.x - line_start.x) * dy - (point.y - line_start.y) * dx;
    cross.abs() / len_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_starts_not_recording() {
        let rec = WaypointRecorder::new();
        assert!(!rec.is_recording());
        assert_eq!(rec.waypoint_count(), 0);
    }

    #[test]
    fn recorder_start_enables_recording() {
        let mut rec = WaypointRecorder::new();
        rec.start();
        assert!(rec.is_recording());
    }

    #[test]
    fn record_position_before_start_is_ignored() {
        let mut rec = WaypointRecorder::new();
        rec.record_position(100.0, 200.0, 0.0);
        assert_eq!(rec.waypoint_count(), 0);
    }

    #[test]
    fn record_position_captures_first_point() {
        let mut rec = WaypointRecorder::new();
        rec.start();
        rec.record_position(100.0, 200.0, 0.0);
        assert_eq!(rec.waypoint_count(), 1);
    }

    #[test]
    fn record_position_skips_nearby_points() {
        let mut rec = WaypointRecorder::new();
        rec.start();
        rec.record_position(100.0, 200.0, 0.0);
        // Move less than MIN_WAYPOINT_DISTANCE (10.0)
        rec.record_position(105.0, 200.0, 0.0);
        assert_eq!(rec.waypoint_count(), 1, "nearby point should be skipped");
    }

    #[test]
    fn record_position_captures_distant_points() {
        let mut rec = WaypointRecorder::new();
        rec.start();
        rec.record_position(100.0, 200.0, 0.0);
        // Move more than MIN_WAYPOINT_DISTANCE
        rec.record_position(200.0, 200.0, 0.0);
        assert_eq!(rec.waypoint_count(), 2);
    }

    #[test]
    fn stop_returns_recorded_waypoints_and_clears() {
        let mut rec = WaypointRecorder::new();
        rec.start();
        rec.record_position(0.0, 0.0, 0.0);
        rec.record_position(100.0, 0.0, 0.0);
        rec.record_position(200.0, 0.0, 0.0);

        let waypoints = rec.stop();
        assert_eq!(waypoints.len(), 3);
        assert!(!rec.is_recording());
        assert_eq!(rec.waypoint_count(), 0);
    }

    #[test]
    fn simplify_path_two_or_fewer_points_unchanged() {
        let empty: Vec<Waypoint> = vec![];
        assert_eq!(simplify_path(&empty, 1.0).len(), 0);

        let one = vec![Waypoint::new(0.0, 0.0, 0.0)];
        assert_eq!(simplify_path(&one, 1.0).len(), 1);

        let two = vec![Waypoint::new(0.0, 0.0, 0.0), Waypoint::new(100.0, 0.0, 0.0)];
        assert_eq!(simplify_path(&two, 1.0).len(), 2);
    }

    #[test]
    fn simplify_path_collinear_points_reduced() {
        // Points along a straight line should be reduced to just endpoints
        let points: Vec<Waypoint> = (0..10)
            .map(|i| Waypoint::new(i as f32 * 10.0, 0.0, 0.0))
            .collect();
        let simplified = simplify_path(&points, 1.0);
        assert_eq!(
            simplified.len(),
            2,
            "collinear points should reduce to 2 endpoints"
        );
        assert!((simplified[0].x - 0.0).abs() < f32::EPSILON);
        assert!((simplified[1].x - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn simplify_path_preserves_deviation() {
        // L-shaped path: should keep the corner point
        let points = vec![
            Waypoint::new(0.0, 0.0, 0.0),
            Waypoint::new(100.0, 0.0, 0.0),
            Waypoint::new(100.0, 100.0, 0.0),
        ];
        let simplified = simplify_path(&points, 1.0);
        assert_eq!(simplified.len(), 3, "corner point should be preserved");
    }

    #[test]
    fn simplify_path_large_epsilon_keeps_only_endpoints() {
        let points = vec![
            Waypoint::new(0.0, 0.0, 0.0),
            Waypoint::new(50.0, 10.0, 0.0),
            Waypoint::new(100.0, 0.0, 0.0),
        ];
        // With a large enough epsilon, the middle point is within tolerance
        let simplified = simplify_path(&points, 100.0);
        assert_eq!(simplified.len(), 2);
    }

    #[test]
    fn simplify_path_reduces_point_count() {
        // Zig-zag path with some collinear segments
        let mut points = Vec::new();
        for i in 0..20 {
            let x = i as f32 * 10.0;
            let y = if i % 5 == 0 { 50.0 } else { 0.0 };
            points.push(Waypoint::new(x, y, 0.0));
        }
        let simplified = simplify_path(&points, 5.0);
        assert!(
            simplified.len() < points.len(),
            "simplified ({}) should have fewer points than original ({})",
            simplified.len(),
            points.len()
        );
        // First and last should be preserved
        assert!((simplified[0].x - points[0].x).abs() < f32::EPSILON);
        assert!((simplified.last().unwrap().x - points.last().unwrap().x).abs() < f32::EPSILON);
    }
}

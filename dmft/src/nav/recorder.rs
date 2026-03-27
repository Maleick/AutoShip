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
        let elapsed = self.start_time.take().map(|s| s.elapsed()).unwrap_or_default();
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

    for (i, wp) in waypoints.iter().enumerate().skip(1).take(waypoints.len() - 2) {
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

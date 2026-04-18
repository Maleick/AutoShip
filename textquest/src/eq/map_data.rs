/// Map data population module.
///
/// This module provides structures and functions for populating map data from zone files,
/// organizing terrain, spawn points, portals, and collision blockers into a structured format.
use anyhow::{Context, Result};
use std::path::Path;

use super::map_parser::{MapLine, MapPoint, ZoneMap, load_zone_map};

/// Represents a terrain segment (line on the map).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainSegment {
    /// Start X coordinate
    pub x1: f32,
    /// Start Y coordinate
    pub y1: f32,
    /// Start Z coordinate
    pub z1: f32,
    /// End X coordinate
    pub x2: f32,
    /// End Y coordinate
    pub y2: f32,
    /// End Z coordinate
    pub z2: f32,
    /// Red color component
    pub r: u8,
    /// Green color component
    pub g: u8,
    /// Blue color component
    pub b: u8,
    /// Layer identifier
    pub layer: u8,
}

impl TerrainSegment {
    /// Create a terrain segment from a map line.
    #[must_use]
    pub fn from_map_line(line: &MapLine) -> Self {
        Self {
            x1: line.x1,
            y1: line.y1,
            z1: line.z1,
            x2: line.x2,
            y2: line.y2,
            z2: line.z2,
            r: line.r,
            g: line.g,
            b: line.b,
            layer: line.layer,
        }
    }

    /// Returns the length of this terrain segment.
    #[must_use]
    pub fn length(&self) -> f32 {
        let dx = self.x2 - self.x1;
        let dy = self.y2 - self.y1;
        let dz = self.z2 - self.z1;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Returns the midpoint of this terrain segment.
    #[must_use]
    pub fn midpoint(&self) -> (f32, f32, f32) {
        (
            (self.x1 + self.x2) / 2.0,
            (self.y1 + self.y2) / 2.0,
            (self.z1 + self.z2) / 2.0,
        )
    }

    /// Returns whether this segment is roughly horizontal (small Z delta).
    #[must_use]
    pub fn is_roughly_horizontal(&self) -> bool {
        let dz = (self.z2 - self.z1).abs();
        dz < 1.0
    }
}

/// Represents a point of interest on the map (spawn location, portal, etc).
#[derive(Debug, Clone)]
pub struct MapDataPoint {
    /// X coordinate
    pub x: f32,
    /// Y coordinate
    pub y: f32,
    /// Z coordinate
    pub z: f32,
    /// Red color component
    pub r: u8,
    /// Green color component
    pub g: u8,
    /// Blue color component
    pub b: u8,
    /// Display size
    pub size: u8,
    /// Label/name
    pub label: String,
    /// Point classification
    pub point_type: MapPointType,
    /// Layer identifier
    pub layer: u8,
}

/// Classification of map points by type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapPointType {
    /// Zone boundary/line to adjacent zone
    Portal,
    /// Spawn location for NPCs or quest targets
    SpawnPoint,
    /// Point of interest or landmark
    Landmark,
    /// Unclassified point
    Unknown,
}

impl MapPointType {
    /// Classify a point based on its label.
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        let lower = label.to_ascii_lowercase();

        if lower.contains("zone") || lower.contains("line") || lower.contains("portal") {
            MapPointType::Portal
        } else if lower.contains("spawn") {
            MapPointType::SpawnPoint
        } else if lower.contains("camp")
            || lower.contains("landmark")
            || lower.contains("point")
            || lower.contains("spot")
        {
            MapPointType::Landmark
        } else {
            MapPointType::Unknown
        }
    }

    /// Human-readable name for this point type.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            MapPointType::Portal => "portal",
            MapPointType::SpawnPoint => "spawn",
            MapPointType::Landmark => "landmark",
            MapPointType::Unknown => "unknown",
        }
    }
}

impl MapDataPoint {
    /// Create a map data point from a map point, classifying it automatically.
    #[must_use]
    pub fn from_map_point(point: &MapPoint) -> Self {
        let point_type = MapPointType::from_label(&point.label);
        Self {
            x: point.x,
            y: point.y,
            z: point.z,
            r: point.r,
            g: point.g,
            b: point.b,
            size: point.size,
            label: point.label.clone(),
            point_type,
            layer: point.layer,
        }
    }

    /// Create a map data point with explicit type.
    #[must_use]
    pub fn with_type(mut self, point_type: MapPointType) -> Self {
        self.point_type = point_type;
        self
    }

    /// Returns the distance from this point to another (in XY plane).
    #[must_use]
    pub fn distance_xy(&self, other: &MapDataPoint) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Returns the distance from this point to another (in 3D).
    #[must_use]
    pub fn distance_3d(&self, other: &MapDataPoint) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Collision blocker or obstacle on the map.
#[derive(Debug, Clone)]
pub struct Blocker {
    /// Blocker name/description
    pub name: String,
    /// Minimum X coordinate of bounding box
    pub min_x: f32,
    /// Maximum X coordinate of bounding box
    pub max_x: f32,
    /// Minimum Y coordinate of bounding box
    pub min_y: f32,
    /// Maximum Y coordinate of bounding box
    pub max_y: f32,
    /// Minimum Z coordinate of bounding box
    pub min_z: f32,
    /// Maximum Z coordinate of bounding box
    pub max_z: f32,
    /// Blocker type
    pub blocker_type: BlockerType,
}

/// Classification of blockers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockerType {
    /// Geometry that blocks movement
    Geometry,
    /// Water or liquid terrain
    Water,
    /// Lava or hazardous terrain
    Lava,
    /// Pit or fall zone
    Pit,
    /// Unclassified blocker
    Unknown,
}

impl BlockerType {
    /// Human-readable name for this blocker type.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            BlockerType::Geometry => "geometry",
            BlockerType::Water => "water",
            BlockerType::Lava => "lava",
            BlockerType::Pit => "pit",
            BlockerType::Unknown => "unknown",
        }
    }
}

impl Blocker {
    /// Returns the volume of this blocker.
    #[must_use]
    pub fn volume(&self) -> f32 {
        let width = (self.max_x - self.min_x).max(0.0);
        let depth = (self.max_y - self.min_y).max(0.0);
        let height = (self.max_z - self.min_z).max(0.0);
        width * depth * height
    }

    /// Returns whether a point is inside this blocker.
    #[must_use]
    pub fn contains_point(&self, x: f32, y: f32, z: f32) -> bool {
        x >= self.min_x
            && x <= self.max_x
            && y >= self.min_y
            && y <= self.max_y
            && z >= self.min_z
            && z <= self.max_z
    }

    /// Returns the center of this blocker.
    #[must_use]
    pub fn center(&self) -> (f32, f32, f32) {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
            (self.min_z + self.max_z) / 2.0,
        )
    }
}

/// Populated map data for a zone.
#[derive(Debug, Clone)]
pub struct PopulatedMapData {
    /// Zone short name
    pub zone_name: String,
    /// All terrain segments
    pub terrain: Vec<TerrainSegment>,
    /// All points of interest
    pub points: Vec<MapDataPoint>,
    /// Portals/zone connections
    pub portals: Vec<MapDataPoint>,
    /// Spawn points
    pub spawns: Vec<MapDataPoint>,
    /// Landmarks and POIs
    pub landmarks: Vec<MapDataPoint>,
    /// Collision blockers
    pub blockers: Vec<Blocker>,
    /// Bounding box min X
    pub bounds_min_x: f32,
    /// Bounding box max X
    pub bounds_max_x: f32,
    /// Bounding box min Y
    pub bounds_min_y: f32,
    /// Bounding box max Y
    pub bounds_max_y: f32,
    /// Bounding box min Z
    pub bounds_min_z: f32,
    /// Bounding box max Z
    pub bounds_max_z: f32,
}

impl PopulatedMapData {
    /// Populate map data from a zone map.
    #[must_use]
    pub fn from_zone_map(zone_map: &ZoneMap) -> Self {
        // Convert terrain lines
        let terrain: Vec<TerrainSegment> = zone_map
            .lines
            .iter()
            .map(TerrainSegment::from_map_line)
            .collect();

        // Classify and organize points
        let mut points = Vec::new();
        let mut portals = Vec::new();
        let mut spawns = Vec::new();
        let mut landmarks = Vec::new();

        for map_point in &zone_map.points {
            let point = MapDataPoint::from_map_point(map_point);
            match point.point_type {
                MapPointType::Portal => portals.push(point.clone()),
                MapPointType::SpawnPoint => spawns.push(point.clone()),
                MapPointType::Landmark => landmarks.push(point.clone()),
                MapPointType::Unknown => {}
            }
            points.push(point);
        }

        // Blockers would normally be derived from terrain geometry analysis,
        // but for now we leave this for future enhancement.
        let blockers = Vec::new();

        Self {
            zone_name: zone_map.name.clone(),
            terrain,
            points,
            portals,
            spawns,
            landmarks,
            blockers,
            bounds_min_x: zone_map.bounds.min_x,
            bounds_max_x: zone_map.bounds.max_x,
            bounds_min_y: zone_map.bounds.min_y,
            bounds_max_y: zone_map.bounds.max_y,
            bounds_min_z: zone_map.bounds.min_z,
            bounds_max_z: zone_map.bounds.max_z,
        }
    }

    /// Populate map data from zone files in a directory.
    ///
    /// # Errors
    ///
    /// Returns an error if loading or parsing fails.
    pub fn load_from_dir(map_dir: &Path, zone_name: &str) -> Result<Self> {
        let zone_map = load_zone_map(map_dir, zone_name).context("failed to load zone map")?;
        Ok(Self::from_zone_map(&zone_map))
    }

    /// Returns statistics about the populated map data.
    #[must_use]
    pub fn statistics(&self) -> MapDataStatistics {
        MapDataStatistics {
            zone_name: self.zone_name.clone(),
            terrain_segment_count: self.terrain.len(),
            point_count: self.points.len(),
            portal_count: self.portals.len(),
            spawn_count: self.spawns.len(),
            landmark_count: self.landmarks.len(),
            blocker_count: self.blockers.len(),
            total_terrain_length: self.terrain.iter().map(|t| t.length()).sum(),
            bounds_width: self.bounds_max_x - self.bounds_min_x,
            bounds_height: self.bounds_max_y - self.bounds_min_y,
        }
    }

    /// Validates the map data for consistency and correctness.
    ///
    /// Returns a list of validation errors (empty if valid).
    pub fn validate(&self) -> Vec<MapValidationError> {
        let mut errors = Vec::new();

        // Check for empty zone name
        if self.zone_name.is_empty() {
            errors.push(MapValidationError::EmptyZoneName);
        }

        // Check point counts match
        let classified_count = self.portals.len() + self.spawns.len() + self.landmarks.len();
        let unclassified_count = self
            .points
            .iter()
            .filter(|p| p.point_type == MapPointType::Unknown)
            .count();
        if classified_count + unclassified_count != self.points.len() {
            errors.push(MapValidationError::PointCountMismatch {
                expected: self.points.len(),
                got: classified_count + unclassified_count,
            });
        }

        // Check for invalid bounding box
        if self.bounds_min_x > self.bounds_max_x {
            errors.push(MapValidationError::InvalidBoundsX {
                min_x: self.bounds_min_x,
                max_x: self.bounds_max_x,
            });
        }
        if self.bounds_min_y > self.bounds_max_y {
            errors.push(MapValidationError::InvalidBoundsY {
                min_y: self.bounds_min_y,
                max_y: self.bounds_max_y,
            });
        }

        // Check for NaN values in terrain
        for (idx, terrain) in self.terrain.iter().enumerate() {
            if !terrain.x1.is_finite()
                || !terrain.y1.is_finite()
                || !terrain.z1.is_finite()
                || !terrain.x2.is_finite()
                || !terrain.y2.is_finite()
                || !terrain.z2.is_finite()
            {
                errors.push(MapValidationError::NonFiniteTerrainValue { segment_index: idx });
            }
        }

        // Check for NaN values in points
        for (idx, point) in self.points.iter().enumerate() {
            if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite() {
                errors.push(MapValidationError::NonFinitePointValue { point_index: idx });
            }
        }

        errors
    }

    /// Returns whether the map data is valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

/// Statistics about populated map data.
#[derive(Debug, Clone)]
pub struct MapDataStatistics {
    /// Zone name
    pub zone_name: String,
    /// Number of terrain segments
    pub terrain_segment_count: usize,
    /// Total number of points
    pub point_count: usize,
    /// Number of portals
    pub portal_count: usize,
    /// Number of spawns
    pub spawn_count: usize,
    /// Number of landmarks
    pub landmark_count: usize,
    /// Number of blockers
    pub blocker_count: usize,
    /// Total length of all terrain segments
    pub total_terrain_length: f32,
    /// Width of bounding box
    pub bounds_width: f32,
    /// Height of bounding box
    pub bounds_height: f32,
}

/// Validation error in map data.
#[derive(Debug, Clone, PartialEq)]
pub enum MapValidationError {
    /// Zone name is empty
    EmptyZoneName,
    /// Point count mismatch between classified and total
    PointCountMismatch { expected: usize, got: usize },
    /// Invalid bounding box coordinates
    InvalidBoundsX { min_x: f32, max_x: f32 },
    /// Invalid bounding box coordinates Y
    InvalidBoundsY { min_y: f32, max_y: f32 },
    /// Non-finite (NaN or Inf) value in terrain
    NonFiniteTerrainValue { segment_index: usize },
    /// Non-finite (NaN or Inf) value in point
    NonFinitePointValue { point_index: usize },
}

impl std::fmt::Display for MapValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapValidationError::EmptyZoneName => write!(f, "Zone name is empty"),
            MapValidationError::PointCountMismatch { expected, got } => {
                write!(f, "Point count mismatch: expected {expected}, got {got}")
            }
            MapValidationError::InvalidBoundsX { min_x, max_x } => {
                write!(f, "Invalid X bounds: min {min_x} > max {max_x}")
            }
            MapValidationError::InvalidBoundsY { min_y, max_y } => {
                write!(f, "Invalid Y bounds: min {min_y} > max {max_y}")
            }
            MapValidationError::NonFiniteTerrainValue { segment_index } => {
                write!(f, "Non-finite value in terrain segment {segment_index}")
            }
            MapValidationError::NonFinitePointValue { point_index } => {
                write!(f, "Non-finite value in point {point_index}")
            }
        }
    }
}

impl std::error::Error for MapValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_segment_from_map_line() {
        let line = super::super::map_parser::MapLine {
            x1: 1.0,
            y1: 2.0,
            z1: 3.0,
            x2: 4.0,
            y2: 5.0,
            z2: 6.0,
            r: 255,
            g: 128,
            b: 64,
            layer: 1,
        };
        let terrain = TerrainSegment::from_map_line(&line);
        assert_eq!(terrain.x1, 1.0);
        assert_eq!(terrain.r, 255);
        assert_eq!(terrain.layer, 1);
    }

    #[test]
    fn terrain_segment_length() {
        let segment = TerrainSegment {
            x1: 0.0,
            y1: 0.0,
            z1: 0.0,
            x2: 3.0,
            y2: 4.0,
            z2: 0.0,
            r: 255,
            g: 255,
            b: 255,
            layer: 0,
        };
        assert!((segment.length() - 5.0).abs() < 0.01);
    }

    #[test]
    fn terrain_segment_midpoint() {
        let segment = TerrainSegment {
            x1: 0.0,
            y1: 0.0,
            z1: 0.0,
            x2: 10.0,
            y2: 10.0,
            z2: 10.0,
            r: 255,
            g: 255,
            b: 255,
            layer: 0,
        };
        let (x, y, z) = segment.midpoint();
        assert_eq!(x, 5.0);
        assert_eq!(y, 5.0);
        assert_eq!(z, 5.0);
    }

    #[test]
    fn terrain_segment_is_roughly_horizontal() {
        let horizontal = TerrainSegment {
            x1: 0.0,
            y1: 0.0,
            z1: 0.0,
            x2: 10.0,
            y2: 10.0,
            z2: 0.5,
            r: 255,
            g: 255,
            b: 255,
            layer: 0,
        };
        assert!(horizontal.is_roughly_horizontal());

        let vertical = TerrainSegment {
            x1: 0.0,
            y1: 0.0,
            z1: 0.0,
            x2: 10.0,
            y2: 10.0,
            z2: 10.0,
            r: 255,
            g: 255,
            b: 255,
            layer: 0,
        };
        assert!(!vertical.is_roughly_horizontal());
    }

    #[test]
    fn map_point_type_from_label_portal() {
        assert_eq!(MapPointType::from_label("zone line"), MapPointType::Portal);
        assert_eq!(
            MapPointType::from_label("freeport zone"),
            MapPointType::Portal
        );
        assert_eq!(
            MapPointType::from_label("portal exit"),
            MapPointType::Portal
        );
    }

    #[test]
    fn map_point_type_from_label_spawn() {
        assert_eq!(MapPointType::from_label("spawn"), MapPointType::SpawnPoint);
        assert_eq!(
            MapPointType::from_label("orc spawn"),
            MapPointType::SpawnPoint
        );
    }

    #[test]
    fn map_point_type_from_label_landmark() {
        assert_eq!(MapPointType::from_label("camp"), MapPointType::Landmark);
        assert_eq!(MapPointType::from_label("landmark"), MapPointType::Landmark);
        assert_eq!(
            MapPointType::from_label("point of interest"),
            MapPointType::Landmark
        );
    }

    #[test]
    fn map_point_type_from_label_unknown() {
        assert_eq!(MapPointType::from_label("random"), MapPointType::Unknown);
    }

    #[test]
    fn map_point_type_name() {
        assert_eq!(MapPointType::Portal.name(), "portal");
        assert_eq!(MapPointType::SpawnPoint.name(), "spawn");
        assert_eq!(MapPointType::Landmark.name(), "landmark");
        assert_eq!(MapPointType::Unknown.name(), "unknown");
    }

    #[test]
    fn map_data_point_distance_xy() {
        let p1 = MapDataPoint {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            r: 255,
            g: 255,
            b: 255,
            size: 1,
            label: "p1".into(),
            point_type: MapPointType::Unknown,
            layer: 0,
        };
        let p2 = MapDataPoint {
            x: 3.0,
            y: 4.0,
            z: 100.0,
            r: 255,
            g: 255,
            b: 255,
            size: 1,
            label: "p2".into(),
            point_type: MapPointType::Unknown,
            layer: 0,
        };
        assert!((p1.distance_xy(&p2) - 5.0).abs() < 0.01);
    }

    #[test]
    fn map_data_point_distance_3d() {
        let p1 = MapDataPoint {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            r: 255,
            g: 255,
            b: 255,
            size: 1,
            label: "p1".into(),
            point_type: MapPointType::Unknown,
            layer: 0,
        };
        let p2 = MapDataPoint {
            x: 1.0,
            y: 1.0,
            z: 1.0,
            r: 255,
            g: 255,
            b: 255,
            size: 1,
            label: "p2".into(),
            point_type: MapPointType::Unknown,
            layer: 0,
        };
        let dist = p1.distance_3d(&p2);
        let expected = (3.0f32).sqrt();
        assert!((dist - expected).abs() < 0.01);
    }

    #[test]
    fn blocker_volume() {
        let blocker = Blocker {
            name: "test".into(),
            min_x: 0.0,
            max_x: 10.0,
            min_y: 0.0,
            max_y: 20.0,
            min_z: 0.0,
            max_z: 5.0,
            blocker_type: BlockerType::Geometry,
        };
        assert_eq!(blocker.volume(), 1000.0);
    }

    #[test]
    fn blocker_contains_point() {
        let blocker = Blocker {
            name: "test".into(),
            min_x: 0.0,
            max_x: 10.0,
            min_y: 0.0,
            max_y: 10.0,
            min_z: 0.0,
            max_z: 10.0,
            blocker_type: BlockerType::Geometry,
        };
        assert!(blocker.contains_point(5.0, 5.0, 5.0));
        assert!(!blocker.contains_point(15.0, 5.0, 5.0));
        assert!(!blocker.contains_point(5.0, 15.0, 5.0));
        assert!(!blocker.contains_point(5.0, 5.0, 15.0));
    }

    #[test]
    fn blocker_center() {
        let blocker = Blocker {
            name: "test".into(),
            min_x: 0.0,
            max_x: 10.0,
            min_y: 0.0,
            max_y: 20.0,
            min_z: 0.0,
            max_z: 4.0,
            blocker_type: BlockerType::Geometry,
        };
        let (x, y, z) = blocker.center();
        assert_eq!(x, 5.0);
        assert_eq!(y, 10.0);
        assert_eq!(z, 2.0);
    }

    #[test]
    fn populated_map_data_from_zone_map() {
        let zone_map = super::super::map_parser::ZoneMap {
            name: "testzone".into(),
            lines: vec![super::super::map_parser::MapLine {
                x1: 0.0,
                y1: 0.0,
                z1: 0.0,
                x2: 10.0,
                y2: 10.0,
                z2: 0.0,
                r: 255,
                g: 255,
                b: 255,
                layer: 0,
            }],
            points: vec![super::super::map_parser::MapPoint {
                x: 5.0,
                y: 5.0,
                z: 0.0,
                r: 0,
                g: 255,
                b: 0,
                size: 2,
                label: "zone line".into(),
                layer: 0,
            }],
            bounds: super::super::map_parser::MapBounds {
                min_x: 0.0,
                max_x: 10.0,
                min_y: 0.0,
                max_y: 10.0,
                min_z: 0.0,
                max_z: 10.0,
            },
        };

        let data = PopulatedMapData::from_zone_map(&zone_map);
        assert_eq!(data.zone_name, "testzone");
        assert_eq!(data.terrain.len(), 1);
        assert_eq!(data.points.len(), 1);
        assert_eq!(data.portals.len(), 1);
        assert_eq!(data.spawns.len(), 0);
        assert_eq!(data.landmarks.len(), 0);
    }

    #[test]
    fn populated_map_data_statistics() {
        let zone_map = super::super::map_parser::ZoneMap {
            name: "testzone".into(),
            lines: vec![super::super::map_parser::MapLine {
                x1: 0.0,
                y1: 0.0,
                z1: 0.0,
                x2: 10.0,
                y2: 10.0,
                z2: 0.0,
                r: 255,
                g: 255,
                b: 255,
                layer: 0,
            }],
            points: vec![super::super::map_parser::MapPoint {
                x: 5.0,
                y: 5.0,
                z: 0.0,
                r: 0,
                g: 255,
                b: 0,
                size: 2,
                label: "zone line".into(),
                layer: 0,
            }],
            bounds: super::super::map_parser::MapBounds {
                min_x: 0.0,
                max_x: 10.0,
                min_y: 0.0,
                max_y: 10.0,
                min_z: 0.0,
                max_z: 10.0,
            },
        };

        let data = PopulatedMapData::from_zone_map(&zone_map);
        let stats = data.statistics();
        assert_eq!(stats.zone_name, "testzone");
        assert_eq!(stats.terrain_segment_count, 1);
        assert_eq!(stats.point_count, 1);
        assert_eq!(stats.portal_count, 1);
    }

    #[test]
    fn populated_map_data_validate_valid() {
        let zone_map = super::super::map_parser::ZoneMap {
            name: "testzone".into(),
            lines: vec![super::super::map_parser::MapLine {
                x1: 0.0,
                y1: 0.0,
                z1: 0.0,
                x2: 10.0,
                y2: 10.0,
                z2: 0.0,
                r: 255,
                g: 255,
                b: 255,
                layer: 0,
            }],
            points: vec![super::super::map_parser::MapPoint {
                x: 5.0,
                y: 5.0,
                z: 0.0,
                r: 0,
                g: 255,
                b: 0,
                size: 2,
                label: "zone line".into(),
                layer: 0,
            }],
            bounds: super::super::map_parser::MapBounds {
                min_x: 0.0,
                max_x: 10.0,
                min_y: 0.0,
                max_y: 10.0,
                min_z: 0.0,
                max_z: 10.0,
            },
        };

        let data = PopulatedMapData::from_zone_map(&zone_map);
        assert!(data.is_valid());
        assert!(data.validate().is_empty());
    }

    #[test]
    fn populated_map_data_validate_empty_zone_name() {
        let data = PopulatedMapData {
            zone_name: String::new(),
            terrain: Vec::new(),
            points: Vec::new(),
            portals: Vec::new(),
            spawns: Vec::new(),
            landmarks: Vec::new(),
            blockers: Vec::new(),
            bounds_min_x: 0.0,
            bounds_max_x: 10.0,
            bounds_min_y: 0.0,
            bounds_max_y: 10.0,
            bounds_min_z: 0.0,
            bounds_max_z: 10.0,
        };
        let errors = data.validate();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, MapValidationError::EmptyZoneName))
        );
    }

    #[test]
    fn populated_map_data_validate_invalid_bounds() {
        let data = PopulatedMapData {
            zone_name: "test".into(),
            terrain: Vec::new(),
            points: Vec::new(),
            portals: Vec::new(),
            spawns: Vec::new(),
            landmarks: Vec::new(),
            blockers: Vec::new(),
            bounds_min_x: 10.0,
            bounds_max_x: 0.0,
            bounds_min_y: 0.0,
            bounds_max_y: 10.0,
            bounds_min_z: 0.0,
            bounds_max_z: 10.0,
        };
        let errors = data.validate();
        assert!(!errors.is_empty());
    }

    #[test]
    fn blocker_type_name() {
        assert_eq!(BlockerType::Geometry.name(), "geometry");
        assert_eq!(BlockerType::Water.name(), "water");
        assert_eq!(BlockerType::Lava.name(), "lava");
        assert_eq!(BlockerType::Pit.name(), "pit");
        assert_eq!(BlockerType::Unknown.name(), "unknown");
    }
}

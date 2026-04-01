use anyhow::{Context, Result};
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// A line segment from an EQ map file (L line).
#[derive(Debug, Clone)]
pub struct MapLine {
    /// Start X coordinate.
    pub x1: f32,
    /// Start Y coordinate.
    pub y1: f32,
    /// Start Z coordinate.
    pub z1: f32,
    /// End X coordinate.
    pub x2: f32,
    /// End Y coordinate.
    pub y2: f32,
    /// End Z coordinate.
    pub z2: f32,
    /// Red color component (0-255).
    pub r: u8,
    /// Green color component (0-255).
    pub g: u8,
    /// Blue color component (0-255).
    pub b: u8,
}

/// A labeled point from an EQ map file (P line).
#[derive(Debug, Clone)]
pub struct MapPoint {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Z coordinate.
    pub z: f32,
    /// Red color component (0-255).
    pub r: u8,
    /// Green color component (0-255).
    pub g: u8,
    /// Blue color component (0-255).
    pub b: u8,
    /// Display size for the point marker.
    pub size: u8,
    /// Text label for the point (e.g., zone connection name).
    pub label: String,
}

/// All data for a single zone map.
#[derive(Debug, Clone)]
pub struct ZoneMap {
    /// Zone short name.
    pub name: String,
    /// All line segments from the map files.
    pub lines: Vec<MapLine>,
    /// All labeled points from the map files.
    pub points: Vec<MapPoint>,
    /// Bounding box enclosing all map geometry.
    pub bounds: MapBounds,
}

/// Axis-aligned bounding box for the map data.
#[derive(Debug, Clone, Copy)]
pub struct MapBounds {
    /// Minimum X coordinate in the map.
    pub min_x: f32,
    /// Maximum X coordinate in the map.
    pub max_x: f32,
    /// Minimum Y coordinate in the map.
    pub min_y: f32,
    /// Maximum Y coordinate in the map.
    pub max_y: f32,
}

impl MapBounds {
    fn empty() -> Self {
        Self {
            min_x: f32::MAX,
            max_x: f32::MIN,
            min_y: f32::MAX,
            max_y: f32::MIN,
        }
    }

    fn expand(&mut self, x: f32, y: f32) {
        if x < self.min_x {
            self.min_x = x;
        }
        if x > self.max_x {
            self.max_x = x;
        }
        if y < self.min_y {
            self.min_y = y;
        }
        if y > self.max_y {
            self.max_y = y;
        }
    }

    /// Returns the width of the bounding box (X axis).
    #[must_use]
    pub fn width(&self) -> f32 {
        (self.max_x - self.min_x).max(1.0)
    }

    /// Returns the height of the bounding box (Y axis).
    #[must_use]
    pub fn height(&self) -> f32 {
        (self.max_y - self.min_y).max(1.0)
    }

    /// Returns the X coordinate of the bounding box center.
    #[must_use]
    pub fn center_x(&self) -> f32 {
        (self.min_x + self.max_x) / 2.0
    }

    /// Returns the Y coordinate of the bounding box center.
    #[must_use]
    pub fn center_y(&self) -> f32 {
        (self.min_y + self.max_y) / 2.0
    }
}

/// Load a zone map from all layer files in the given directory.
/// Looks for `zone.txt`, `zone_1.txt`, `zone_2.txt`, `zone_3.txt`.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn load_zone_map(map_dir: &Path, zone_name: &str) -> Result<ZoneMap> {
    let mut lines = Vec::new();
    let mut points = Vec::new();

    let zone_lower = zone_name.to_lowercase();

    // Load layers 0-3
    let suffixes = ["", "_1", "_2", "_3"];
    for suffix in &suffixes {
        let filename = format!("{zone_lower}{suffix}.txt");
        let path = map_dir.join(&filename);
        if path.exists() {
            parse_map_file(&path, &mut lines, &mut points)
                .with_context(|| format!("parsing {filename}"))?;
        }
    }

    // Compute bounding box from all line endpoints
    let mut bounds = MapBounds::empty();
    for line in &lines {
        bounds.expand(line.x1, line.y1);
        bounds.expand(line.x2, line.y2);
    }
    for point in &points {
        bounds.expand(point.x, point.y);
    }

    lines.sort_by(|a, b| {
        let a_depth = (a.z1 + a.z2) * 0.5;
        let b_depth = (b.z1 + b.z2) * 0.5;
        a_depth.partial_cmp(&b_depth).unwrap_or(Ordering::Equal)
    });
    points.sort_by(|a, b| {
        b.z.partial_cmp(&a.z)
            .unwrap_or(Ordering::Equal)
            .then(a.size.cmp(&b.size))
    });

    // If no data was loaded, set bounds to origin
    if bounds.min_x == f32::MAX {
        bounds = MapBounds {
            min_x: -100.0,
            max_x: 100.0,
            min_y: -100.0,
            max_y: 100.0,
        };
    }

    Ok(ZoneMap {
        name: zone_name.to_string(),
        lines,
        points,
        bounds,
    })
}

fn parse_map_file(path: &Path, lines: &mut Vec<MapLine>, points: &mut Vec<MapPoint>) -> Result<()> {
    let file = File::open(path)?;
    let mut bad_lines = 0usize;

    for (index, line_result) in BufReader::new(file).lines().enumerate() {
        let raw = line_result?;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let parsed = if trimmed.starts_with('L') {
            if let Some(ml) = parse_l_line(trimmed) {
                lines.push(ml);
                true
            } else {
                false
            }
        } else if trimmed.starts_with('P') {
            if let Some(mp) = parse_p_line(trimmed) {
                points.push(mp);
                true
            } else {
                false
            }
        } else {
            true
        };

        if !parsed {
            tracing::warn!(
                file = %path.display(),
                line = index + 1,
                raw_line = trimmed,
                "Ignoring malformed map row"
            );
            bad_lines = bad_lines.saturating_add(1);
        }
    }

    if bad_lines > 0 {
        tracing::info!(
            file = %path.display(),
            bad_lines,
            "Map file loaded with malformed rows"
        );
    }

    Ok(())
}

fn parse_u8_channel(value: &str) -> Option<u8> {
    value
        .parse::<u16>()
        .ok()
        .filter(|&v| v <= u8::MAX as u16)
        .and_then(|v| u8::try_from(v).ok())
}

fn parse_l_line(line: &str) -> Option<MapLine> {
    // Format: L x1, y1, z1, x2, y2, z2, r, g, b
    let rest = line[1..].trim();
    let parts: Vec<&str> = rest.splitn(9, ',').map(str::trim).collect();
    if parts.len() != 9 {
        return None;
    }
    Some(MapLine {
        x1: parts[0].parse().ok()?,
        y1: parts[1].parse().ok()?,
        z1: parts[2].parse().ok()?,
        x2: parts[3].parse().ok()?,
        y2: parts[4].parse().ok()?,
        z2: parts[5].parse().ok()?,
        r: parse_u8_channel(parts[6])?,
        g: parse_u8_channel(parts[7])?,
        b: parse_u8_channel(parts[8])?,
    })
}

fn parse_p_line(line: &str) -> Option<MapPoint> {
    // Format: P x, y, z, r, g, b, size, label_text
    // Use splitn(8, ',') so commas in the label are preserved.
    let rest = line[1..].trim();
    let parts: Vec<&str> = rest.splitn(8, ',').map(str::trim).collect();
    if parts.len() != 8 {
        return None;
    }
    // Replace underscores with spaces in label (Brewall convention)
    let label = parts[7].replace('_', " ");
    Some(MapPoint {
        x: parts[0].parse().ok()?,
        y: parts[1].parse().ok()?,
        z: parts[2].parse().ok()?,
        r: parse_u8_channel(parts[3])?,
        g: parse_u8_channel(parts[4])?,
        b: parse_u8_channel(parts[5])?,
        size: parts[6].parse().ok()?,
        label,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_l_line_valid() {
        let line = "L 2881.0, -2022.0, -295.0, 2885.0, -2027.0, -295.0, 128, 255, 0";
        let ml = parse_l_line(line).unwrap();
        assert!((ml.x1 - 2881.0).abs() < 0.01);
        assert!((ml.y1 - (-2022.0)).abs() < 0.01);
        assert_eq!(ml.r, 128);
        assert_eq!(ml.g, 255);
        assert_eq!(ml.b, 0);
    }

    #[test]
    fn parse_p_line_valid() {
        let line = "P 5531.2642, -168.7061, -299.5485, 128, 255, 0, 2, Gargoyle_Island";
        let mp = parse_p_line(line).unwrap();
        assert!((mp.x - 5_531.264).abs() < 0.01);
        assert_eq!(mp.size, 2);
        assert_eq!(mp.label, "Gargoyle Island");
    }

    #[test]
    fn parse_p_line_with_commas_in_label() {
        let line = "P -3710.0, -1594.5, -192.5, 128, 255, 0, 2, Gull_Skytalon_(Named,Roam)";
        let mp = parse_p_line(line).unwrap();
        assert_eq!(mp.label, "Gull Skytalon (Named,Roam)");
    }

    #[test]
    fn parse_l_line_invalid() {
        assert!(parse_l_line("L 1, 2, 3").is_none());
        assert!(parse_l_line("L abc").is_none());
    }

    #[test]
    fn load_zone_map_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let map_path = dir.path().join("testzone.txt");
        let mut f = File::create(&map_path).unwrap();
        writeln!(f, "# Test map").unwrap();
        writeln!(f, "L -100, 0, 0, 100, 0, 0, 200, 200, 200").unwrap();
        writeln!(f, "L 100, 0, 0, 100, 200, 0, 200, 200, 200").unwrap();
        writeln!(f, "P 0, 100, 0, 255, 0, 0, 3, Zone_Line").unwrap();
        drop(f);

        let map = load_zone_map(dir.path(), "testzone").unwrap();
        assert_eq!(map.lines.len(), 2);
        assert_eq!(map.points.len(), 1);
        assert_eq!(map.points[0].label, "Zone Line");
        assert!((map.bounds.min_x - (-100.0)).abs() < 0.01);
        assert!((map.bounds.max_x - 100.0).abs() < 0.01);
    }

    #[test]
    fn load_zone_map_with_points_only() {
        let dir = tempfile::tempdir().unwrap();
        let map_path = dir.path().join("pointzone.txt");
        let mut f = File::create(&map_path).unwrap();
        writeln!(f, "P 10, 20, 0, 255, 0, 0, 2, Test_Label").unwrap();
        drop(f);

        let map = load_zone_map(dir.path(), "pointzone").unwrap();
        assert!(map.lines.is_empty());
        assert_eq!(map.points.len(), 1);
        assert!((map.bounds.min_x - 10.0).abs() < 0.01);
        assert!((map.bounds.max_y - 20.0).abs() < 0.01);
    }

    #[test]
    fn load_zone_map_with_layers() {
        let dir = tempfile::tempdir().unwrap();

        let mut f0 = File::create(dir.path().join("myzone.txt")).unwrap();
        writeln!(f0, "L 0, 0, 0, 100, 0, 0, 255, 255, 255").unwrap();
        drop(f0);

        let mut f1 = File::create(dir.path().join("myzone_1.txt")).unwrap();
        writeln!(f1, "P 50, 50, 0, 0, 255, 0, 2, Merchant_Area").unwrap();
        drop(f1);

        let map = load_zone_map(dir.path(), "myzone").unwrap();
        assert_eq!(map.lines.len(), 1);
        assert_eq!(map.points.len(), 1);
    }

    #[test]
    fn load_zone_map_missing_zone() {
        let dir = tempfile::tempdir().unwrap();
        let map = load_zone_map(dir.path(), "nonexistent").unwrap();
        assert!(map.lines.is_empty());
        assert!(map.points.is_empty());
    }

    #[test]
    fn map_bounds_empty() {
        let bounds = MapBounds::empty();
        assert_eq!(bounds.min_x, f32::MAX);
        assert_eq!(bounds.max_x, f32::MIN);
    }

    #[test]
    fn map_bounds_expand_first_point() {
        let mut bounds = MapBounds::empty();
        bounds.expand(10.0, 20.0);
        assert!((bounds.min_x - 10.0).abs() < f32::EPSILON);
        assert!((bounds.max_x - 10.0).abs() < f32::EPSILON);
        assert!((bounds.min_y - 20.0).abs() < f32::EPSILON);
        assert!((bounds.max_y - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn map_bounds_expand_multiple_points() {
        let mut bounds = MapBounds::empty();
        bounds.expand(-100.0, -50.0);
        bounds.expand(200.0, 150.0);
        bounds.expand(0.0, 0.0);
        assert!((bounds.min_x - (-100.0)).abs() < f32::EPSILON);
        assert!((bounds.max_x - 200.0).abs() < f32::EPSILON);
        assert!((bounds.min_y - (-50.0)).abs() < f32::EPSILON);
        assert!((bounds.max_y - 150.0).abs() < f32::EPSILON);
    }

    #[test]
    fn map_bounds_width_and_height() {
        let mut bounds = MapBounds::empty();
        bounds.expand(-50.0, -30.0);
        bounds.expand(50.0, 70.0);
        assert!((bounds.width() - 100.0).abs() < f32::EPSILON);
        assert!((bounds.height() - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn map_bounds_width_minimum_is_one() {
        let mut bounds = MapBounds::empty();
        bounds.expand(5.0, 10.0);
        // Same point, width/height would be 0, but max(1.0) applies
        assert!((bounds.width() - 1.0).abs() < f32::EPSILON);
        assert!((bounds.height() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn map_bounds_center() {
        let mut bounds = MapBounds::empty();
        bounds.expand(-100.0, -50.0);
        bounds.expand(100.0, 50.0);
        assert!((bounds.center_x()).abs() < f32::EPSILON);
        assert!((bounds.center_y()).abs() < f32::EPSILON);
    }

    #[test]
    fn map_bounds_center_offset() {
        let mut bounds = MapBounds::empty();
        bounds.expand(100.0, 200.0);
        bounds.expand(200.0, 400.0);
        assert!((bounds.center_x() - 150.0).abs() < f32::EPSILON);
        assert!((bounds.center_y() - 300.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_l_line_with_negative_coords() {
        let line = "L -100.5, -200.3, -50.0, 100.5, 200.3, 50.0, 0, 128, 255";
        let ml = parse_l_line(line).unwrap();
        assert!((ml.x1 - (-100.5)).abs() < 0.01);
        assert!((ml.y2 - 200.3).abs() < 0.01);
        assert_eq!(ml.r, 0);
    }

    #[test]
    fn parse_l_line_too_few_fields() {
        assert!(parse_l_line("L 1, 2, 3, 4, 5").is_none());
    }

    #[test]
    fn parse_l_line_invalid_number() {
        assert!(parse_l_line("L abc, 2, 3, 4, 5, 6, 7, 8, 9").is_none());
    }

    #[test]
    fn parse_p_line_too_few_fields() {
        assert!(parse_p_line("P 1, 2, 3, 4, 5").is_none());
    }

    #[test]
    fn parse_p_line_too_many_fields_keeps_unknown_label_comma_text() {
        let mp = parse_p_line("P 1, 2, 3, 4, 5, 6, 7, Zone, extra").unwrap();
        assert_eq!(mp.label, "Zone, extra");
    }

    #[test]
    fn parse_l_line_out_of_range_color() {
        assert!(parse_l_line("L 1, 2, 3, 4, 5, 6, 256, 0, 0").is_none());
    }

    #[test]
    fn parse_p_line_out_of_range_color() {
        assert!(parse_p_line("P 1, 2, 3, 0, 255, -1, 2, Zone").is_none());
    }

    #[test]
    fn parse_p_line_invalid_number() {
        assert!(parse_p_line("P abc, 2, 3, 4, 5, 6, 7, label").is_none());
    }

    #[test]
    fn load_zone_map_ignores_comments_and_blanks() {
        let dir = tempfile::tempdir().unwrap();
        let map_path = dir.path().join("testzone.txt");
        let mut f = File::create(&map_path).unwrap();
        writeln!(f, "# This is a comment").unwrap();
        writeln!(f).unwrap(); // blank line
        writeln!(f, "L 0, 0, 0, 100, 0, 0, 255, 255, 255").unwrap();
        writeln!(f, "# Another comment").unwrap();
        writeln!(f, "L 0, 0, 0, 0, 100, 0, 128, 128, 128").unwrap();
        drop(f);

        let map = load_zone_map(dir.path(), "testzone").unwrap();
        assert_eq!(map.lines.len(), 2);
    }

    #[test]
    fn load_zone_map_empty_map_gets_default_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let map = load_zone_map(dir.path(), "nonexistent").unwrap();
        assert!((map.bounds.min_x - (-100.0)).abs() < f32::EPSILON);
        assert!((map.bounds.max_x - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn load_zone_map_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let map_path = dir.path().join("myzone.txt");
        let mut f = File::create(&map_path).unwrap();
        writeln!(f, "L 0, 0, 0, 50, 50, 0, 255, 255, 255").unwrap();
        drop(f);

        // Zone name with uppercase should load lowercase file
        let map = load_zone_map(dir.path(), "MyZone").unwrap();
        assert_eq!(map.lines.len(), 1);
    }

    #[test]
    fn map_line_struct_fields() {
        let line = MapLine {
            x1: 1.0,
            y1: 2.0,
            z1: 3.0,
            x2: 4.0,
            y2: 5.0,
            z2: 6.0,
            r: 255,
            g: 128,
            b: 0,
        };
        assert!((line.z2 - 6.0).abs() < f32::EPSILON);
        assert_eq!(line.g, 128);
    }

    #[test]
    fn map_point_struct_fields() {
        let point = MapPoint {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            r: 255,
            g: 0,
            b: 128,
            size: 3,
            label: "Test Point".into(),
        };
        assert_eq!(point.label, "Test Point");
        assert_eq!(point.size, 3);
    }

    #[test]
    fn zone_map_name_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let map = load_zone_map(dir.path(), "Crushbone").unwrap();
        assert_eq!(map.name, "Crushbone");
    }
}

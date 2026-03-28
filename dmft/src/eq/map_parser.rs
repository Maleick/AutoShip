use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// A line segment from an EQ map file (L line).
#[derive(Debug, Clone)]
pub struct MapLine {
    pub x1: f32,
    pub y1: f32,
    pub z1: f32,
    pub x2: f32,
    pub y2: f32,
    pub z2: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A labeled point from an EQ map file (P line).
#[derive(Debug, Clone)]
pub struct MapPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub size: u8,
    pub label: String,
}

/// All data for a single zone map.
#[derive(Debug, Clone)]
pub struct ZoneMap {
    pub name: String,
    pub lines: Vec<MapLine>,
    pub points: Vec<MapPoint>,
    pub bounds: MapBounds,
}

/// Axis-aligned bounding box for the map data.
#[derive(Debug, Clone, Copy)]
pub struct MapBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
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

    pub fn width(&self) -> f32 {
        (self.max_x - self.min_x).max(1.0)
    }

    pub fn height(&self) -> f32 {
        (self.max_y - self.min_y).max(1.0)
    }

    pub fn center_x(&self) -> f32 {
        (self.min_x + self.max_x) / 2.0
    }

    pub fn center_y(&self) -> f32 {
        (self.min_y + self.max_y) / 2.0
    }
}

/// Load a zone map from all layer files in the given directory.
/// Looks for `zone.txt`, `zone_1.txt`, `zone_2.txt`, `zone_3.txt`.
pub fn load_zone_map(map_dir: &Path, zone_name: &str) -> Result<ZoneMap> {
    let mut lines = Vec::new();
    let mut points = Vec::new();

    let zone_lower = zone_name.to_lowercase();

    // Load layers 0-3
    let suffixes = ["", "_1", "_2", "_3"];
    for suffix in &suffixes {
        let filename = format!("{}{}.txt", zone_lower, suffix);
        let path = map_dir.join(&filename);
        if path.exists() {
            parse_map_file(&path, &mut lines, &mut points)
                .with_context(|| format!("parsing {}", filename))?;
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

fn parse_map_file(
    path: &Path,
    lines: &mut Vec<MapLine>,
    points: &mut Vec<MapPoint>,
) -> Result<()> {
    let file = File::open(path)?;
    for line_result in BufReader::new(file).lines() {
        let raw = line_result?;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('L')
            && let Some(ml) = parse_l_line(trimmed)
        {
            lines.push(ml);
        } else if trimmed.starts_with('P')
            && let Some(mp) = parse_p_line(trimmed)
        {
            points.push(mp);
        }
    }
    Ok(())
}

fn parse_l_line(line: &str) -> Option<MapLine> {
    // Format: L x1, y1, z1, x2, y2, z2, r, g, b
    let rest = line[1..].trim();
    let parts: Vec<&str> = rest.splitn(9, ',').map(|s| s.trim()).collect();
    if parts.len() < 9 {
        return None;
    }
    Some(MapLine {
        x1: parts[0].parse().ok()?,
        y1: parts[1].parse().ok()?,
        z1: parts[2].parse().ok()?,
        x2: parts[3].parse().ok()?,
        y2: parts[4].parse().ok()?,
        z2: parts[5].parse().ok()?,
        r: parts[6].parse().ok()?,
        g: parts[7].parse().ok()?,
        b: parts[8].parse().ok()?,
    })
}

fn parse_p_line(line: &str) -> Option<MapPoint> {
    // Format: P x, y, z, r, g, b, size, label_text
    // Use splitn(8, ',') so commas in the label are preserved.
    let rest = line[1..].trim();
    let parts: Vec<&str> = rest.splitn(8, ',').map(|s| s.trim()).collect();
    if parts.len() < 8 {
        return None;
    }
    // Replace underscores with spaces in label (Brewall convention)
    let label = parts[7].replace('_', " ");
    Some(MapPoint {
        x: parts[0].parse().ok()?,
        y: parts[1].parse().ok()?,
        z: parts[2].parse().ok()?,
        r: parts[3].parse().ok()?,
        g: parts[4].parse().ok()?,
        b: parts[5].parse().ok()?,
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
        assert!((mp.x - 5531.2642).abs() < 0.01);
        assert_eq!(mp.size, 2);
        assert_eq!(mp.label, "Gargoyle Island");
    }

    #[test]
    fn parse_p_line_with_commas_in_label() {
        let line =
            "P -3710.0, -1594.5, -192.5, 128, 255, 0, 2, Gull_Skytalon_(Named,Roam)";
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
}

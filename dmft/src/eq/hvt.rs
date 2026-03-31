use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Priority level for a high-value target.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HvtPriority {
    Critical,
    High,
    Medium,
    Low,
}

/// A single high-value target entry from the watchlist.
#[derive(Debug, Clone, Deserialize)]
pub struct HvtTarget {
    pub name: String,
    pub zone: String,
    pub priority: HvtPriority,
    #[serde(default)]
    pub alert_discord: bool,
    #[serde(default)]
    pub note: String,
}

/// Container for the full HVT watchlist, indexed by lowercase name for fast lookup.
#[derive(Debug, Clone)]
pub struct HvtWatchlist {
    targets: HashMap<String, HvtTarget>,
}

#[derive(Deserialize)]
struct WatchlistFile {
    targets: Vec<HvtTarget>,
}

impl HvtWatchlist {
    /// Load the watchlist from a TOML file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let file: WatchlistFile = toml::from_str(&content)?;
        let mut targets = HashMap::with_capacity(file.targets.len());
        for t in file.targets {
            targets.insert(t.name.to_lowercase(), t);
        }
        Ok(Self { targets })
    }

    /// Check if a spawn name matches an HVT entry (case-insensitive).
    pub fn is_hvt(&self, name: &str) -> Option<&HvtTarget> {
        self.targets.get(&name.to_lowercase())
    }

    /// Number of targets in the watchlist.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether the watchlist is empty.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Iterate over all targets.
    pub fn iter(&self) -> impl Iterator<Item = &HvtTarget> {
        self.targets.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sample_toml() -> &'static str {
        r#"
[[targets]]
name = "Emperor Crush"
zone = "crushbone"
priority = "high"
alert_discord = true
note = "Drops Belt of the River"

[[targets]]
name = "Lord Nagafen"
zone = "nagafen"
priority = "critical"
alert_discord = true
note = "Dragon raid target"

[[targets]]
name = "the Tangrin"
zone = "highkeep"
priority = "medium"
alert_discord = false
note = "Drops Mithril Two-Handed Sword"
"#
    }

    #[test]
    fn test_load_watchlist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hvt.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(sample_toml().as_bytes()).unwrap();

        let wl = HvtWatchlist::load(&path).unwrap();
        assert_eq!(wl.len(), 3);
        assert!(!wl.is_empty());
    }

    #[test]
    fn test_is_hvt_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hvt.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(sample_toml().as_bytes()).unwrap();

        let wl = HvtWatchlist::load(&path).unwrap();

        // Exact match
        let hit = wl.is_hvt("Emperor Crush");
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().zone, "crushbone");
        assert_eq!(hit.unwrap().priority, HvtPriority::High);

        // Case-insensitive
        assert!(wl.is_hvt("emperor crush").is_some());
        assert!(wl.is_hvt("EMPEROR CRUSH").is_some());

        // Miss
        assert!(wl.is_hvt("a moss snake").is_none());
    }

    #[test]
    fn test_hvt_priority_and_discord() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hvt.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(sample_toml().as_bytes()).unwrap();

        let wl = HvtWatchlist::load(&path).unwrap();

        let nagafen = wl.is_hvt("Lord Nagafen").unwrap();
        assert_eq!(nagafen.priority, HvtPriority::Critical);
        assert!(nagafen.alert_discord);

        let tangrin = wl.is_hvt("the Tangrin").unwrap();
        assert_eq!(tangrin.priority, HvtPriority::Medium);
        assert!(!tangrin.alert_discord);
    }

    #[test]
    fn test_empty_watchlist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hvt.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"targets = []\n").unwrap();

        let wl = HvtWatchlist::load(&path).unwrap();
        assert!(wl.is_empty());
        assert_eq!(wl.len(), 0);
        assert!(wl.is_hvt("anything").is_none());
    }

    #[test]
    fn test_iter_all_targets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hvt.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(sample_toml().as_bytes()).unwrap();

        let wl = HvtWatchlist::load(&path).unwrap();
        let names: Vec<_> = wl.iter().map(|t| t.name.clone()).collect();
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn test_hvt_note_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hvt.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(sample_toml().as_bytes()).unwrap();

        let wl = HvtWatchlist::load(&path).unwrap();
        let crush = wl.is_hvt("Emperor Crush").unwrap();
        assert_eq!(crush.note, "Drops Belt of the River");
    }

    #[test]
    fn test_hvt_default_fields() {
        let toml = r#"
[[targets]]
name = "Test"
zone = "testzone"
priority = "low"
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hvt.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(toml.as_bytes()).unwrap();

        let wl = HvtWatchlist::load(&path).unwrap();
        let t = wl.is_hvt("Test").unwrap();
        assert!(!t.alert_discord); // default false
        assert!(t.note.is_empty()); // default empty
        assert_eq!(t.priority, HvtPriority::Low);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = HvtWatchlist::load(Path::new("/nonexistent/hvt.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_priority_all_variants() {
        let variants = ["critical", "high", "medium", "low"];
        for v in variants {
            let toml = format!(
                r#"[[targets]]
name = "T"
zone = "z"
priority = "{v}"
"#
            );
            let parsed: WatchlistFile = toml::from_str(&toml).unwrap();
            assert_eq!(parsed.targets.len(), 1);
        }
    }
}

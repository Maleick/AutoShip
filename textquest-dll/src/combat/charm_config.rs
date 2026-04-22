//! Charm/pet configuration loader with hot-reload support.
//!
//! Reads TOML files from `config/charm/` (overridden via `TEXTQUEST_CHARM_CONFIG_DIR`).
//! Two config layers:
//!   - `global.toml`   — defaults applied to every class
//!   - `<class>.toml`  — per-class overrides merged on top of global
//!
//! Hot-reload: call `CharmConfigLoader::poll_reload` each game tick (or at any
//! convenient low-frequency interval).  The loader re-reads files only when
//! their modification timestamp has advanced, so the happy-path cost is a single
//! `metadata()` syscall per file.

use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result};
use serde::Deserialize;

// ─── Environment override ──────────────────────────────────────────────────────

const CHARM_CONFIG_ENV_VAR: &str = "TEXTQUEST_CHARM_CONFIG_DIR";

fn configured_charm_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(CHARM_CONFIG_ENV_VAR) {
        return Ok(PathBuf::from(dir));
    }
    Ok(std::env::current_dir()
        .context("Failed to resolve current working directory for charm config lookup")?
        .join("config/charm"))
}

// ─── Schema ───────────────────────────────────────────────────────────────────

/// Pet behaviour stance during combat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetBehaviorMode {
    /// Pet attacks aggressively; will break CC if not directed.
    Aggressive,
    /// Pet assists on the main-assist target; avoids mezzed mobs.
    #[default]
    Balanced,
    /// Pet stays near owner and only counter-attacks.
    Defensive,
}

/// Ordered spell priority list entry for pet/charm casting.
#[derive(Debug, Clone, Deserialize)]
pub struct PetSpellPriority {
    /// Spell name as it appears in the spell book.
    pub name: String,
    /// Lower number = higher priority (1 is highest).
    pub priority: u8,
    /// Optional condition expression (same syntax as rotation conditions).
    #[serde(default)]
    pub condition: Option<String>,
}

/// Affinity preference for charmed pets.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PetAffinityPrefs {
    /// Preferred mob types to charm (e.g. `["animal", "humanoid"]`).
    #[serde(default)]
    pub preferred_types: Vec<String>,
    /// Mob types to avoid charming even when they are in range.
    #[serde(default)]
    pub avoid_types: Vec<String>,
    /// Minimum mob level eligible for charm.
    #[serde(default)]
    pub min_level: Option<u8>,
    /// Maximum mob level eligible for charm.
    #[serde(default)]
    pub max_level: Option<u8>,
}

/// Re-charm thresholds controlling when the automation re-applies charm.
#[derive(Debug, Clone, Deserialize)]
pub struct ReCharmThresholds {
    /// Re-charm when remaining charm duration falls below this many ticks.
    #[serde(default = "default_recharge_ticks")]
    pub recharge_below_ticks: u32,
    /// Percentage of pet HP below which we disengage the pet before re-charm.
    #[serde(default = "default_pet_hp_safety_pct")]
    pub pet_hp_safety_pct: f32,
    /// Minimum mana percentage required before attempting a re-charm cast.
    #[serde(default = "default_min_mana_pct")]
    pub min_mana_pct: f32,
}

impl Default for ReCharmThresholds {
    fn default() -> Self {
        Self {
            recharge_below_ticks: default_recharge_ticks(),
            pet_hp_safety_pct: default_pet_hp_safety_pct(),
            min_mana_pct: default_min_mana_pct(),
        }
    }
}

fn default_recharge_ticks() -> u32 {
    60
}
fn default_pet_hp_safety_pct() -> f32 {
    40.0
}
fn default_min_mana_pct() -> f32 {
    20.0
}

/// Root charm/pet configuration struct.  All fields are optional so a partial
/// TOML file (even an empty one) is valid; missing fields inherit defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct CharmConfig {
    /// Pet behaviour stance.
    #[serde(default)]
    pub pet_behavior_mode: PetBehaviorMode,

    /// Re-charm timing thresholds.
    #[serde(default)]
    pub recharge: ReCharmThresholds,

    /// Affinity preferences for choosing which mob to charm.
    #[serde(default)]
    pub affinity: PetAffinityPrefs,

    /// Ordered list of spells the pet should use (when pet command is
    /// supported).
    #[serde(default)]
    pub pet_spell_priorities: Vec<PetSpellPriority>,

    /// Whether to automatically re-charm a broken charm pet.
    #[serde(default = "default_true")]
    pub auto_recharm: bool,

    /// Whether to send the pet to attack after charm succeeds.
    #[serde(default = "default_true")]
    pub auto_send_pet: bool,

    /// Maximum number of mobs to charm simultaneously (enchanter group charm).
    #[serde(default = "default_max_charmed")]
    pub max_charmed: u8,
}

fn default_true() -> bool {
    true
}
fn default_max_charmed() -> u8 {
    1
}

impl Default for CharmConfig {
    fn default() -> Self {
        Self {
            pet_behavior_mode: PetBehaviorMode::default(),
            recharge: ReCharmThresholds::default(),
            affinity: PetAffinityPrefs::default(),
            pet_spell_priorities: Vec::new(),
            auto_recharm: true,
            auto_send_pet: true,
            max_charmed: default_max_charmed(),
        }
    }
}

// ─── Loader ───────────────────────────────────────────────────────────────────

/// Load and merge charm config from a directory.
///
/// Reads `global.toml` first, then merges `<class_name>.toml` on top.
/// Returns the merged config.
pub fn load_from_dir(dir: &Path, class_name: &str) -> Result<CharmConfig> {
    let global_path = dir.join("global.toml");
    let mut merged = load_single_optional(&global_path)?.unwrap_or_default();

    let class_lower = class_name.trim().to_ascii_lowercase();
    if !class_lower.is_empty() {
        let class_path = dir.join(format!("{class_lower}.toml"));
        if let Some(class_cfg) = load_single_optional(&class_path)? {
            merge_into(&mut merged, class_cfg);
        }
    }

    Ok(merged)
}

/// Load charm config for a named class using the configured (or default) directory.
pub fn load_for_class(class_name: &str) -> Result<CharmConfig> {
    let dir = configured_charm_dir()?;
    load_from_dir(&dir, class_name)
}

fn load_single_optional(path: &Path) -> Result<Option<CharmConfig>> {
    if !path.is_file() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read charm config: {}", path.display()))?;
    let config = toml::from_str::<CharmConfig>(&contents)
        .with_context(|| format!("Failed to parse charm config: {}", path.display()))?;
    Ok(Some(config))
}

/// Merge `overlay` fields into `base` — non-default values in `overlay` win.
///
/// Strategy: overlay replaces any field that differs from CharmConfig::default().
/// We do a field-by-field merge so partial TOML files work correctly.
fn merge_into(base: &mut CharmConfig, overlay: CharmConfig) {
    let defaults = CharmConfig::default();

    if overlay.pet_behavior_mode != defaults.pet_behavior_mode {
        base.pet_behavior_mode = overlay.pet_behavior_mode;
    }
    if overlay.recharge.recharge_below_ticks != defaults.recharge.recharge_below_ticks {
        base.recharge.recharge_below_ticks = overlay.recharge.recharge_below_ticks;
    }
    if overlay.recharge.pet_hp_safety_pct != defaults.recharge.pet_hp_safety_pct {
        base.recharge.pet_hp_safety_pct = overlay.recharge.pet_hp_safety_pct;
    }
    if overlay.recharge.min_mana_pct != defaults.recharge.min_mana_pct {
        base.recharge.min_mana_pct = overlay.recharge.min_mana_pct;
    }
    if !overlay.affinity.preferred_types.is_empty() {
        base.affinity.preferred_types = overlay.affinity.preferred_types;
    }
    if !overlay.affinity.avoid_types.is_empty() {
        base.affinity.avoid_types = overlay.affinity.avoid_types;
    }
    if overlay.affinity.min_level.is_some() {
        base.affinity.min_level = overlay.affinity.min_level;
    }
    if overlay.affinity.max_level.is_some() {
        base.affinity.max_level = overlay.affinity.max_level;
    }
    if !overlay.pet_spell_priorities.is_empty() {
        base.pet_spell_priorities = overlay.pet_spell_priorities;
    }
    if !overlay.auto_recharm {
        base.auto_recharm = false;
    }
    if !overlay.auto_send_pet {
        base.auto_send_pet = false;
    }
    if overlay.max_charmed != default_max_charmed() {
        base.max_charmed = overlay.max_charmed;
    }
}

// ─── Hot-reload ───────────────────────────────────────────────────────────────

/// Tracks file modification timestamps for hot-reload detection.
#[derive(Debug)]
struct FileWatch {
    path: PathBuf,
    last_modified: Option<SystemTime>,
}

impl FileWatch {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            last_modified: None,
        }
    }

    /// Returns `true` if the file's mtime has advanced since last check.
    fn has_changed(&mut self) -> bool {
        let Ok(meta) = std::fs::metadata(&self.path) else {
            return false;
        };
        let Ok(mtime) = meta.modified() else {
            return false;
        };
        if self.last_modified.is_none_or(|prev| mtime > prev) {
            self.last_modified = Some(mtime);
            true
        } else {
            false
        }
    }
}

/// Stateful loader with hot-reload support.
///
/// Holds the current merged `CharmConfig` and watches both `global.toml` and
/// the class-specific file for modification.  Call `poll_reload` periodically
/// (e.g., every N game ticks) to refresh when files change on disk.
pub struct CharmConfigLoader {
    dir: PathBuf,
    class_name: String,
    config: CharmConfig,
    global_watch: FileWatch,
    class_watch: FileWatch,
}

impl CharmConfigLoader {
    /// Create a new loader, immediately loading the initial config.
    pub fn new(class_name: impl Into<String>) -> Result<Self> {
        let dir = configured_charm_dir()?;
        Self::new_in_dir(dir, class_name)
    }

    /// Create a loader rooted at a specific directory (useful in tests).
    pub fn new_in_dir(dir: impl Into<PathBuf>, class_name: impl Into<String>) -> Result<Self> {
        let dir: PathBuf = dir.into();
        let class_name: String = class_name.into();

        let config = load_from_dir(&dir, &class_name)?;

        let class_lower = class_name.trim().to_ascii_lowercase();
        let global_watch = FileWatch::new(dir.join("global.toml"));
        let class_watch = FileWatch::new(dir.join(format!("{class_lower}.toml")));

        let mut loader = Self {
            dir,
            class_name,
            config,
            global_watch,
            class_watch,
        };

        // Seed the mtimes so the first poll does not spuriously trigger.
        loader.global_watch.has_changed();
        loader.class_watch.has_changed();

        Ok(loader)
    }

    /// Return a reference to the currently-active config.
    pub fn config(&self) -> &CharmConfig {
        &self.config
    }

    /// Check watched files; reload if either has changed.
    ///
    /// Returns `true` if the config was reloaded.
    pub fn poll_reload(&mut self) -> bool {
        let global_changed = self.global_watch.has_changed();
        let class_changed = self.class_watch.has_changed();

        if global_changed || class_changed {
            match load_from_dir(&self.dir, &self.class_name) {
                Ok(new_cfg) => {
                    self.config = new_cfg;
                    tracing::info!(
                        class = %self.class_name,
                        "Charm config reloaded"
                    );
                    return true;
                }
                Err(e) => {
                    tracing::warn!(
                        class = %self.class_name,
                        error = %e,
                        "Charm config reload failed; keeping previous config"
                    );
                }
            }
        }

        false
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn write_toml(dir: &std::path::Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    // ── Schema / parsing ──────────────────────────────────────────────────────

    #[test]
    fn empty_toml_parses_to_defaults() {
        let cfg: CharmConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.pet_behavior_mode, PetBehaviorMode::Balanced);
        assert!(cfg.auto_recharm);
        assert!(cfg.auto_send_pet);
        assert_eq!(cfg.max_charmed, 1);
        assert_eq!(cfg.recharge.recharge_below_ticks, 60);
        assert!((cfg.recharge.pet_hp_safety_pct - 40.0).abs() < f32::EPSILON);
        assert!((cfg.recharge.min_mana_pct - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn pet_behavior_mode_parses_all_variants() {
        let aggressive: CharmConfig =
            toml::from_str("pet_behavior_mode = \"aggressive\"").unwrap();
        assert_eq!(aggressive.pet_behavior_mode, PetBehaviorMode::Aggressive);

        let balanced: CharmConfig = toml::from_str("pet_behavior_mode = \"balanced\"").unwrap();
        assert_eq!(balanced.pet_behavior_mode, PetBehaviorMode::Balanced);

        let defensive: CharmConfig =
            toml::from_str("pet_behavior_mode = \"defensive\"").unwrap();
        assert_eq!(defensive.pet_behavior_mode, PetBehaviorMode::Defensive);
    }

    #[test]
    fn recharge_thresholds_parse_correctly() {
        let cfg: CharmConfig = toml::from_str(
            r#"
            [recharge]
            recharge_below_ticks = 90
            pet_hp_safety_pct = 35.0
            min_mana_pct = 15.0
        "#,
        )
        .unwrap();
        assert_eq!(cfg.recharge.recharge_below_ticks, 90);
        assert!((cfg.recharge.pet_hp_safety_pct - 35.0).abs() < f32::EPSILON);
        assert!((cfg.recharge.min_mana_pct - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn affinity_prefs_parse_correctly() {
        let cfg: CharmConfig = toml::from_str(
            r#"
            [affinity]
            preferred_types = ["animal", "humanoid"]
            avoid_types = ["undead"]
            min_level = 40
            max_level = 62
        "#,
        )
        .unwrap();
        assert_eq!(cfg.affinity.preferred_types, vec!["animal", "humanoid"]);
        assert_eq!(cfg.affinity.avoid_types, vec!["undead"]);
        assert_eq!(cfg.affinity.min_level, Some(40));
        assert_eq!(cfg.affinity.max_level, Some(62));
    }

    #[test]
    fn pet_spell_priorities_parse_correctly() {
        let cfg: CharmConfig = toml::from_str(
            r#"
            [[pet_spell_priorities]]
            name = "Burnout"
            priority = 1

            [[pet_spell_priorities]]
            name = "Rune"
            priority = 2
            condition = "pet_hp_below_80"
        "#,
        )
        .unwrap();
        assert_eq!(cfg.pet_spell_priorities.len(), 2);
        assert_eq!(cfg.pet_spell_priorities[0].name, "Burnout");
        assert_eq!(cfg.pet_spell_priorities[1].priority, 2);
        assert_eq!(
            cfg.pet_spell_priorities[1].condition.as_deref(),
            Some("pet_hp_below_80")
        );
    }

    // ── Loader: global + class merge ──────────────────────────────────────────

    #[test]
    fn load_from_dir_returns_defaults_when_no_files() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = load_from_dir(dir.path(), "enchanter").unwrap();
        assert_eq!(cfg.pet_behavior_mode, PetBehaviorMode::Balanced);
    }

    #[test]
    fn load_from_dir_reads_global_toml() {
        let dir = tempfile::tempdir().unwrap();
        write_toml(
            dir.path(),
            "global.toml",
            "pet_behavior_mode = \"aggressive\"\nmax_charmed = 2",
        );
        let cfg = load_from_dir(dir.path(), "magician").unwrap();
        assert_eq!(cfg.pet_behavior_mode, PetBehaviorMode::Aggressive);
        assert_eq!(cfg.max_charmed, 2);
    }

    #[test]
    fn load_from_dir_class_overrides_global() {
        let dir = tempfile::tempdir().unwrap();
        write_toml(
            dir.path(),
            "global.toml",
            "pet_behavior_mode = \"aggressive\"\nmax_charmed = 2",
        );
        write_toml(
            dir.path(),
            "enchanter.toml",
            "pet_behavior_mode = \"defensive\"\nmax_charmed = 3",
        );
        let cfg = load_from_dir(dir.path(), "enchanter").unwrap();
        assert_eq!(cfg.pet_behavior_mode, PetBehaviorMode::Defensive);
        assert_eq!(cfg.max_charmed, 3);
    }

    #[test]
    fn load_from_dir_class_name_is_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        write_toml(dir.path(), "enchanter.toml", "max_charmed = 3");
        let cfg = load_from_dir(dir.path(), "Enchanter").unwrap();
        assert_eq!(cfg.max_charmed, 3);
    }

    #[test]
    fn load_from_dir_empty_class_name_skips_class_file() {
        let dir = tempfile::tempdir().unwrap();
        write_toml(dir.path(), "global.toml", "max_charmed = 2");
        let cfg = load_from_dir(dir.path(), "").unwrap();
        assert_eq!(cfg.max_charmed, 2);
    }

    #[test]
    fn load_from_dir_invalid_toml_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        write_toml(dir.path(), "global.toml", "not = valid [toml garbage ][");
        let result = load_from_dir(dir.path(), "enchanter");
        assert!(result.is_err());
    }

    // ── merge_into ────────────────────────────────────────────────────────────

    #[test]
    fn merge_into_does_not_overwrite_with_defaults() {
        let mut base = CharmConfig {
            pet_behavior_mode: PetBehaviorMode::Aggressive,
            max_charmed: 4,
            ..Default::default()
        };
        let overlay = CharmConfig::default(); // all defaults
        merge_into(&mut base, overlay);
        assert_eq!(base.pet_behavior_mode, PetBehaviorMode::Aggressive);
        assert_eq!(base.max_charmed, 4);
    }

    #[test]
    fn merge_into_affinity_lists_replace_when_non_empty() {
        let mut base = CharmConfig::default();
        base.affinity.preferred_types = vec!["animal".into()];

        let mut overlay = CharmConfig::default();
        overlay.affinity.preferred_types = vec!["humanoid".into(), "giant".into()];
        merge_into(&mut base, overlay);

        assert_eq!(base.affinity.preferred_types, vec!["humanoid", "giant"]);
    }

    // ── Hot-reload ────────────────────────────────────────────────────────────

    #[test]
    fn charm_config_loader_loads_initial_config() {
        let dir = tempfile::tempdir().unwrap();
        write_toml(dir.path(), "global.toml", "max_charmed = 2");
        let loader = CharmConfigLoader::new_in_dir(dir.path(), "wizard").unwrap();
        assert_eq!(loader.config().max_charmed, 2);
    }

    #[test]
    fn poll_reload_returns_false_when_nothing_changed() {
        let dir = tempfile::tempdir().unwrap();
        write_toml(dir.path(), "global.toml", "max_charmed = 2");
        let mut loader = CharmConfigLoader::new_in_dir(dir.path(), "wizard").unwrap();
        assert!(!loader.poll_reload());
        assert_eq!(loader.config().max_charmed, 2);
    }

    #[test]
    fn poll_reload_detects_file_change_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        write_toml(dir.path(), "global.toml", "max_charmed = 2");
        let mut loader = CharmConfigLoader::new_in_dir(dir.path(), "wizard").unwrap();
        assert_eq!(loader.config().max_charmed, 2);

        // Simulate a file change by bumping mtime forward.
        // We update loader's watch to pretend it's stale.
        loader.global_watch.last_modified = Some(SystemTime::UNIX_EPOCH);
        write_toml(dir.path(), "global.toml", "max_charmed = 5");
        assert!(loader.poll_reload());
        assert_eq!(loader.config().max_charmed, 5);
    }

    #[test]
    fn poll_reload_keeps_old_config_on_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        write_toml(dir.path(), "global.toml", "max_charmed = 2");
        let mut loader = CharmConfigLoader::new_in_dir(dir.path(), "wizard").unwrap();

        // Force stale mtime so poll_reload attempts a reload.
        loader.global_watch.last_modified = Some(SystemTime::UNIX_EPOCH);
        write_toml(dir.path(), "global.toml", "not_valid = [[[");
        // Should not return true (parse failed), config unchanged.
        let reloaded = loader.poll_reload();
        assert!(!reloaded);
        assert_eq!(loader.config().max_charmed, 2);
    }

    // ── FileWatch ─────────────────────────────────────────────────────────────

    #[test]
    fn file_watch_returns_false_for_missing_file() {
        let mut watch = FileWatch::new(PathBuf::from("/nonexistent/file.toml"));
        assert!(!watch.has_changed());
    }

    #[test]
    fn file_watch_returns_true_on_first_existing_file_check() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut watch = FileWatch::new(tmp.path().to_path_buf());
        assert!(watch.has_changed());
        // Second call should return false (mtime hasn't changed).
        assert!(!watch.has_changed());
    }
}

//! Portal database — canonical EQ port spell registry for travel planning.
//!
//! Covers Wizard, Druid, Mage, and other port-capable classes. The database
//! loads from `config/portals/portals.toml` and supports hot-reload.
//!
//! # Usage
//!
//! ```rust,ignore
//! let db = PortalDatabase::load_default()?;
//! let portals = db.lookup(12, 34, "northkarana"); // class_id, min_level, dest zone
//! ```

use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ── EQ class IDs (matches SpawnData::class_id) ─────────────────────────────
/// EQ class ID for Druid (6).
pub const CLASS_DRUID: u8 = 6;
/// EQ class ID for Wizard (12).
pub const CLASS_WIZARD: u8 = 12;
/// EQ class ID for Mage (13).
pub const CLASS_MAGE: u8 = 13;
/// EQ class ID for Necromancer (11).
pub const CLASS_NECROMANCER: u8 = 11;
/// EQ class ID for Shadow Knight (5).
pub const CLASS_SHADOW_KNIGHT: u8 = 5;

// ── Types ───────────────────────────────────────────────────────────────────

/// The type of portal mechanic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortalKind {
    /// Wizard translocate/port spell — single target or group.
    WizardPort,
    /// Druid port (ring/succor) spell.
    DruidPort,
    /// Druid succor emergency port (same zone, safe spot).
    DruidSuccor,
    /// Mage spell of teleportation (sends target, not caster).
    MagePort,
    /// Necromancer shadow step / gate.
    NecroGate,
    /// Shadow Knight gate or skeletal port.
    SkPort,
    /// Spire/translocator NPC — no spell, clickable object.
    Spire,
}

/// A single canonical EQ portal spell / teleport mechanic entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Portal {
    /// Unique stable identifier (e.g., `wiz_port_northkarana`).
    pub id: String,
    /// Display name shown to the operator.
    pub name: String,
    /// EQ class ID of the caster (matches `SpawnData::class_id`).
    pub class_id: u8,
    /// Minimum level required to cast this spell.
    pub min_level: u8,
    /// Destination zone short name (e.g., `northkarana`).
    pub destination_zone: String,
    /// Approximate safe landing coordinates in the destination zone `[x, y, z]`.
    /// `null` if the landing is zone-default / unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing_coords: Option<[f32; 3]>,
    /// Portal mechanic kind.
    pub kind: PortalKind,
    /// Recast cooldown in seconds (0 = no cooldown beyond mana).
    #[serde(default)]
    pub cooldown_secs: u32,
    /// Whether this spell can port the whole group (true) or only the caster.
    #[serde(default = "default_true")]
    pub group_port: bool,
    /// EQ spell name as it appears in the spell book / gem (for scripting).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spell_name: Option<String>,
    /// Numeric spell ID from EQ spell data (for direct gem-clicking).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spell_id: Option<u32>,
}

fn default_true() -> bool {
    true
}

/// Safe camp meeting point inside a zone — where the group gathers after porting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneSafeCamp {
    /// Zone short name.
    pub zone: String,
    /// Safe camp coordinates `[x, y, z]`.
    pub coords: [f32; 3],
    /// Optional description (e.g., "near zone-in from WC").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// On-disk TOML format for the portal database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalDatabaseFile {
    /// All portal entries.
    #[serde(default)]
    pub portals: Vec<Portal>,
    /// Per-zone safe camp meeting points.
    #[serde(default)]
    pub safe_camps: Vec<ZoneSafeCamp>,
}

/// In-memory portal database with hot-reload support.
pub struct PortalDatabase {
    /// All loaded portals.
    portals: Vec<Portal>,
    /// Per-zone safe camps.
    safe_camps: Vec<ZoneSafeCamp>,
    /// Path this database was loaded from.
    path: PathBuf,
    /// Last-modified time at load (for hot-reload detection).
    last_modified: Option<SystemTime>,
}

impl PortalDatabase {
    // ── Construction ────────────────────────────────────────────────────────

    /// Default config path: `config/portals/portals.toml`.
    #[must_use]
    pub fn default_path() -> PathBuf {
        Path::new("config/portals/portals.toml").to_path_buf()
    }

    /// Load from the default path, falling back to built-in defaults if the
    /// file does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be parsed.
    pub fn load_default() -> Result<Self> {
        Self::load_or_default(Self::default_path())
    }

    /// Load from `path`, falling back to built-in defaults if absent.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be parsed.
    pub fn load_or_default(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if path.exists() {
            Self::load(&path)
        } else {
            Ok(Self::from_builtin_defaults(path))
        }
    }

    /// Load from a specific TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read portal database: {}", path.display()))?;
        let file: PortalDatabaseFile = toml::from_str(&raw)
            .with_context(|| format!("Failed to parse portal database: {}", path.display()))?;
        let last_modified = path.metadata().ok().and_then(|m| m.modified().ok());
        Self::validate(&file.portals)?;
        Ok(Self {
            portals: file.portals,
            safe_camps: file.safe_camps,
            path,
            last_modified,
        })
    }

    /// Construct from the built-in canonical portal list (no file required).
    #[must_use]
    pub fn from_builtin_defaults(path: impl Into<PathBuf>) -> Self {
        let file = builtin_defaults();
        Self {
            portals: file.portals,
            safe_camps: file.safe_camps,
            path: path.into(),
            last_modified: None,
        }
    }

    // ── Hot-reload ───────────────────────────────────────────────────────────

    /// Reload if the backing file has been modified since last load.
    /// Returns `true` if the database was reloaded.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be parsed.
    pub fn reload_if_changed(&mut self) -> Result<bool> {
        if !self.path.exists() {
            return Ok(false);
        }
        let current_mtime = self.path.metadata().ok().and_then(|m| m.modified().ok());
        if current_mtime == self.last_modified {
            return Ok(false);
        }
        *self = Self::load(&self.path)?;
        Ok(true)
    }

    // ── Validation ───────────────────────────────────────────────────────────

    fn validate(portals: &[Portal]) -> Result<()> {
        for p in portals {
            if p.id.is_empty() {
                anyhow::bail!("Portal entry has empty id");
            }
            if p.destination_zone.is_empty() {
                anyhow::bail!("Portal '{}' has empty destination_zone", p.id);
            }
        }
        Ok(())
    }

    // ── Lookups ──────────────────────────────────────────────────────────────

    /// All portals in the database.
    #[must_use]
    pub fn all(&self) -> &[Portal] {
        &self.portals
    }

    /// All portals available to a caster of `class_id` at `level`.
    #[must_use]
    pub fn portals_for_class(&self, class_id: u8, level: u8) -> Vec<&Portal> {
        self.portals
            .iter()
            .filter(|p| p.class_id == class_id && p.min_level <= level)
            .collect()
    }

    /// Find the best portal to `destination_zone` for a caster of `class_id`
    /// at `level`. Prefers the lowest min_level match (earliest learnable spell
    /// that reaches the destination — typically the most reliable cast).
    ///
    /// Returns `None` if no portal reaches the destination for this class/level.
    #[must_use]
    pub fn best_portal_to(
        &self,
        class_id: u8,
        level: u8,
        destination_zone: &str,
    ) -> Option<&Portal> {
        let dest = destination_zone.trim().to_ascii_lowercase();
        self.portals
            .iter()
            .filter(|p| {
                p.class_id == class_id
                    && p.min_level <= level
                    && p.destination_zone.to_ascii_lowercase() == dest
            })
            .min_by_key(|p| p.min_level)
    }

    /// All portals that reach `destination_zone` regardless of class, filtered
    /// to casters at or above `level`.
    #[must_use]
    pub fn portals_to_zone(&self, destination_zone: &str, level: u8) -> Vec<&Portal> {
        let dest = destination_zone.trim().to_ascii_lowercase();
        self.portals
            .iter()
            .filter(|p| {
                p.min_level <= level
                    && p.destination_zone.to_ascii_lowercase() == dest
            })
            .collect()
    }

    /// Safe camp for `zone_name`, if one is defined.
    #[must_use]
    pub fn safe_camp_for(&self, zone_name: &str) -> Option<&ZoneSafeCamp> {
        let zone = zone_name.trim().to_ascii_lowercase();
        self.safe_camps
            .iter()
            .find(|c| c.zone.to_ascii_lowercase() == zone)
    }

    /// All safe camps.
    #[must_use]
    pub fn safe_camps(&self) -> &[ZoneSafeCamp] {
        &self.safe_camps
    }

    /// Save the current database to its backing TOML file (creates parent dirs).
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create portal config dir: {}", parent.display())
            })?;
        }
        let file = PortalDatabaseFile {
            portals: self.portals.clone(),
            safe_camps: self.safe_camps.clone(),
        };
        let toml_str = toml::to_string_pretty(&file).context("Failed to serialize portal database")?;
        std::fs::write(&self.path, toml_str)
            .with_context(|| format!("Failed to write portal database: {}", self.path.display()))?;
        Ok(())
    }
}

// ── Built-in canonical portal data ─────────────────────────────────────────

/// Returns the built-in canonical EQ portal database (Classic–Velious era).
///
/// Sources: EQ Casters' Realm, EQ Magelo spell data, player wikis.
/// Zone short names match EQ's internal zone file names.
#[must_use]
pub fn builtin_defaults() -> PortalDatabaseFile {
    PortalDatabaseFile {
        portals: vec![
            // ── Wizard Ports ─────────────────────────────────────────────
            // Antonica / Faydwer / Odus
            Portal {
                id: "wiz_port_northkarana".into(),
                name: "Alterations of the Nexus (North Karana)".into(),
                class_id: CLASS_WIZARD,
                min_level: 20,
                destination_zone: "northkarana".into(),
                landing_coords: Some([-382.0, -1376.0, -3.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Alterations of the Nexus".into()),
                spell_id: Some(1561),
            },
            Portal {
                id: "wiz_port_commonlands".into(),
                name: "Velocity of Vesshan (Commonlands)".into(),
                class_id: CLASS_WIZARD,
                min_level: 16,
                destination_zone: "commons".into(),
                landing_coords: Some([-1184.0, 331.0, -51.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Velocity of Vesshan".into()),
                spell_id: Some(1546),
            },
            Portal {
                id: "wiz_port_lavastorm".into(),
                name: "Fist of Ixiblat (Lavastorm)".into(),
                class_id: CLASS_WIZARD,
                min_level: 24,
                destination_zone: "lavastorm".into(),
                landing_coords: Some([-494.0, 1034.0, -12.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Fist of Ixiblat".into()),
                spell_id: Some(1566),
            },
            Portal {
                id: "wiz_port_feerrott".into(),
                name: "Zephyr of Brell (Feerott)".into(),
                class_id: CLASS_WIZARD,
                min_level: 34,
                destination_zone: "feerrott".into(),
                landing_coords: Some([-1140.0, 1100.0, 10.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Zephyr of Brell".into()),
                spell_id: Some(1576),
            },
            Portal {
                id: "wiz_port_highkeep".into(),
                name: "Gate of Highkeep (High Keep)".into(),
                class_id: CLASS_WIZARD,
                min_level: 20,
                destination_zone: "highkeep".into(),
                landing_coords: Some([-118.0, 6.0, 4.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Gate of Highkeep".into()),
                spell_id: Some(1559),
            },
            Portal {
                id: "wiz_port_erudsxing".into(),
                name: "Velocity of Erudin (Erud's Crossing)".into(),
                class_id: CLASS_WIZARD,
                min_level: 20,
                destination_zone: "erudsxing".into(),
                landing_coords: Some([400.0, 400.0, 0.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Velocity of Erudin".into()),
                spell_id: Some(1560),
            },
            Portal {
                id: "wiz_port_qeynos".into(),
                name: "Banishment of the Pantheon (Qeynos Hills)".into(),
                class_id: CLASS_WIZARD,
                min_level: 16,
                destination_zone: "qeytoqrg".into(),
                landing_coords: Some([-1202.0, -204.0, -1.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Banishment of the Pantheon".into()),
                spell_id: Some(1547),
            },
            Portal {
                id: "wiz_port_butcherblock".into(),
                name: "Egress of Butcherblock (Butcherblock)".into(),
                class_id: CLASS_WIZARD,
                min_level: 20,
                destination_zone: "butcher".into(),
                landing_coords: Some([-1198.0, -2674.0, 1.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Egress of Butcherblock".into()),
                spell_id: Some(1562),
            },
            Portal {
                id: "wiz_port_timorous".into(),
                name: "Skeighten's Portal (Timorous Deep)".into(),
                class_id: CLASS_WIZARD,
                min_level: 44,
                destination_zone: "timorous".into(),
                landing_coords: Some([2300.0, -5000.0, 5.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Skeighten's Portal".into()),
                spell_id: Some(1579),
            },
            Portal {
                id: "wiz_port_wakening".into(),
                name: "Ensnarement of the Wakening Land (Wakening Lands)".into(),
                class_id: CLASS_WIZARD,
                min_level: 54,
                destination_zone: "wakening".into(),
                landing_coords: Some([2532.0, -1830.0, -49.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Ensnarement of the Wakening Land".into()),
                spell_id: Some(2447),
            },
            Portal {
                id: "wiz_port_iceclad".into(),
                name: "Knothotep's Portal (Iceclad Ocean)".into(),
                class_id: CLASS_WIZARD,
                min_level: 52,
                destination_zone: "iceclad".into(),
                landing_coords: Some([-4108.0, 556.0, -139.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Knothotep's Portal".into()),
                spell_id: Some(2446),
            },
            Portal {
                id: "wiz_port_thurgadina".into(),
                name: "Sunset Home (Thurgadin A — EW access)".into(),
                class_id: CLASS_WIZARD,
                min_level: 56,
                destination_zone: "thurgadina".into(),
                landing_coords: Some([0.0, 0.0, 0.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Sunset Home".into()),
                spell_id: Some(2452),
            },
            Portal {
                id: "wiz_port_velketor".into(),
                name: "Vaziac's Portal (Velketor's Labyrinth)".into(),
                class_id: CLASS_WIZARD,
                min_level: 58,
                destination_zone: "velketor".into(),
                landing_coords: Some([0.0, 0.0, 0.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Vaziac's Portal".into()),
                spell_id: Some(2454),
            },
            // Wizard group translocate spells (Velious+)
            Portal {
                id: "wiz_tlocate_iceclad".into(),
                name: "Translocate: Iceclad (group)".into(),
                class_id: CLASS_WIZARD,
                min_level: 54,
                destination_zone: "iceclad".into(),
                landing_coords: Some([-4108.0, 556.0, -139.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Translocate: Iceclad".into()),
                spell_id: Some(2632),
            },
            Portal {
                id: "wiz_tlocate_wakening".into(),
                name: "Translocate: Wakening Lands (group)".into(),
                class_id: CLASS_WIZARD,
                min_level: 56,
                destination_zone: "wakening".into(),
                landing_coords: Some([2532.0, -1830.0, -49.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Translocate: Wakening Lands".into()),
                spell_id: Some(2634),
            },
            Portal {
                id: "wiz_tlocate_nexus".into(),
                name: "Translocate: Nexus (group, Luclin)".into(),
                class_id: CLASS_WIZARD,
                min_level: 59,
                destination_zone: "nexus".into(),
                landing_coords: Some([0.0, 0.0, 0.0]),
                kind: PortalKind::WizardPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Translocate: Nexus".into()),
                spell_id: Some(3229),
            },

            // ── Druid Ports ──────────────────────────────────────────────
            Portal {
                id: "dru_port_northkarana".into(),
                name: "Circle of Summer (North Karana)".into(),
                class_id: CLASS_DRUID,
                min_level: 29,
                destination_zone: "northkarana".into(),
                landing_coords: Some([-1690.0, 1600.0, -3.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Circle of Summer".into()),
                spell_id: Some(2271),
            },
            Portal {
                id: "dru_port_commonlands".into(),
                name: "Circle of Commons (Commonlands)".into(),
                class_id: CLASS_DRUID,
                min_level: 19,
                destination_zone: "commons".into(),
                landing_coords: Some([-1400.0, -800.0, -51.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Circle of Commons".into()),
                spell_id: Some(2261),
            },
            Portal {
                id: "dru_port_southkarana".into(),
                name: "Circle of Faydark (South Karana)".into(),
                class_id: CLASS_DRUID,
                min_level: 14,
                destination_zone: "southkarana".into(),
                landing_coords: Some([-1080.0, 250.0, 2.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Circle of Faydark".into()),
                spell_id: Some(2257),
            },
            Portal {
                id: "dru_port_lavastorm".into(),
                name: "Circle of Lavastorm (Lavastorm)".into(),
                class_id: CLASS_DRUID,
                min_level: 24,
                destination_zone: "lavastorm".into(),
                landing_coords: Some([-494.0, 1034.0, -12.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Circle of Lavastorm".into()),
                spell_id: Some(2265),
            },
            Portal {
                id: "dru_port_butcherblock".into(),
                name: "Circle of Butcherblock (Butcherblock)".into(),
                class_id: CLASS_DRUID,
                min_level: 19,
                destination_zone: "butcher".into(),
                landing_coords: Some([-1282.0, -2700.0, 1.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Circle of Butcherblock".into()),
                spell_id: Some(2262),
            },
            Portal {
                id: "dru_port_feerrott".into(),
                name: "Circle of Feerott (Feerott)".into(),
                class_id: CLASS_DRUID,
                min_level: 34,
                destination_zone: "feerrott".into(),
                landing_coords: Some([-1140.0, 1100.0, 10.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Circle of Feerott".into()),
                spell_id: Some(2273),
            },
            Portal {
                id: "dru_port_lfay".into(),
                name: "Circle of Greater Faydark (Lesser Faydark)".into(),
                class_id: CLASS_DRUID,
                min_level: 14,
                destination_zone: "lfaydark".into(),
                landing_coords: Some([-1540.0, 320.0, 0.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Circle of Greater Faydark".into()),
                spell_id: Some(2256),
            },
            Portal {
                id: "dru_port_gfay".into(),
                name: "Circle of Greater Faydark (Greater Faydark)".into(),
                class_id: CLASS_DRUID,
                min_level: 14,
                destination_zone: "gfaydark".into(),
                landing_coords: Some([-1082.0, -560.0, 0.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Circle of Steamfont".into()),
                spell_id: Some(2255),
            },
            Portal {
                id: "dru_port_misty".into(),
                name: "Circle of Misty Thicket (Misty Thicket)".into(),
                class_id: CLASS_DRUID,
                min_level: 9,
                destination_zone: "misty".into(),
                landing_coords: Some([-400.0, 380.0, 0.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Circle of Misty Thicket".into()),
                spell_id: Some(2253),
            },
            Portal {
                id: "dru_port_timorous".into(),
                name: "Ring of Timorous (Timorous Deep)".into(),
                class_id: CLASS_DRUID,
                min_level: 49,
                destination_zone: "timorous".into(),
                landing_coords: Some([2300.0, -5000.0, 5.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Ring of Timorous".into()),
                spell_id: Some(2280),
            },
            Portal {
                id: "dru_port_wakening".into(),
                name: "Wandering Mind (Wakening Lands)".into(),
                class_id: CLASS_DRUID,
                min_level: 54,
                destination_zone: "wakening".into(),
                landing_coords: Some([2532.0, -1830.0, -49.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Wandering Mind".into()),
                spell_id: Some(2441),
            },
            Portal {
                id: "dru_port_iceclad".into(),
                name: "Pine Twilight (Iceclad Ocean)".into(),
                class_id: CLASS_DRUID,
                min_level: 52,
                destination_zone: "iceclad".into(),
                landing_coords: Some([-4108.0, 556.0, -139.0]),
                kind: PortalKind::DruidPort,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Pine Twilight".into()),
                spell_id: Some(2440),
            },
            // Druid Succor spells (emergency evac)
            Portal {
                id: "dru_succor_antonica".into(),
                name: "Succor (Antonica safe zone)".into(),
                class_id: CLASS_DRUID,
                min_level: 30,
                destination_zone: "northkarana".into(),
                landing_coords: None,
                kind: PortalKind::DruidSuccor,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Succor".into()),
                spell_id: Some(1225),
            },
            Portal {
                id: "dru_succor_kunark".into(),
                name: "Succor (Kunark safe zone)".into(),
                class_id: CLASS_DRUID,
                min_level: 44,
                destination_zone: "firiona".into(),
                landing_coords: None,
                kind: PortalKind::DruidSuccor,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Succor: Kunark".into()),
                spell_id: Some(1756),
            },
            Portal {
                id: "dru_succor_velious".into(),
                name: "Succor (Velious safe zone)".into(),
                class_id: CLASS_DRUID,
                min_level: 50,
                destination_zone: "greatdivide".into(),
                landing_coords: None,
                kind: PortalKind::DruidSuccor,
                cooldown_secs: 0,
                group_port: true,
                spell_name: Some("Succor: Velious".into()),
                spell_id: Some(2436),
            },

            // ── Mage Ports (send target, not caster) ────────────────────
            Portal {
                id: "mag_port_elemental".into(),
                name: "Elemental Form: Earth (Plane of Earth access)".into(),
                class_id: CLASS_MAGE,
                min_level: 34,
                destination_zone: "airplane".into(),
                landing_coords: None,
                kind: PortalKind::MagePort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Elemental Form: Earth".into()),
                spell_id: Some(1578),
            },
            Portal {
                id: "mag_port_commons".into(),
                name: "Mage Portal: Commonlands".into(),
                class_id: CLASS_MAGE,
                min_level: 20,
                destination_zone: "commons".into(),
                landing_coords: Some([-1184.0, 331.0, -51.0]),
                kind: PortalKind::MagePort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Translocate".into()),
                spell_id: Some(3244),
            },

            // ── Necro Gate ───────────────────────────────────────────────
            Portal {
                id: "nec_gate".into(),
                name: "Gate (Necromancer self-gate to bind point)".into(),
                class_id: CLASS_NECROMANCER,
                min_level: 9,
                destination_zone: "bind_point".into(), // resolved at runtime
                landing_coords: None,
                kind: PortalKind::NecroGate,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Gate".into()),
                spell_id: Some(234),
            },

            // ── Shadow Knight Gate ───────────────────────────────────────
            Portal {
                id: "sk_gate".into(),
                name: "Gate (Shadow Knight self-gate to bind point)".into(),
                class_id: CLASS_SHADOW_KNIGHT,
                min_level: 9,
                destination_zone: "bind_point".into(),
                landing_coords: None,
                kind: PortalKind::SkPort,
                cooldown_secs: 0,
                group_port: false,
                spell_name: Some("Gate".into()),
                spell_id: Some(234),
            },
        ],
        safe_camps: vec![
            ZoneSafeCamp {
                zone: "northkarana".into(),
                coords: [-382.0, -1376.0, -3.0],
                description: Some("Wizard port-in, open field, safe".into()),
            },
            ZoneSafeCamp {
                zone: "commons".into(),
                coords: [-1184.0, 331.0, -51.0],
                description: Some("Wizard port-in near tunnel entrance".into()),
            },
            ZoneSafeCamp {
                zone: "butcher".into(),
                coords: [-1198.0, -2674.0, 1.0],
                description: Some("Druid/Wiz port-in near docks".into()),
            },
            ZoneSafeCamp {
                zone: "lavastorm".into(),
                coords: [-494.0, 1034.0, -12.0],
                description: Some("Port-in near zone entry".into()),
            },
            ZoneSafeCamp {
                zone: "timorous".into(),
                coords: [2300.0, -5000.0, 5.0],
                description: Some("Wizard port-in, beach area".into()),
            },
            ZoneSafeCamp {
                zone: "iceclad".into(),
                coords: [-4108.0, 556.0, -139.0],
                description: Some("Druid/Wiz Velious port-in".into()),
            },
            ZoneSafeCamp {
                zone: "wakening".into(),
                coords: [2532.0, -1830.0, -49.0],
                description: Some("Druid/Wiz Velious port-in".into()),
            },
        ],
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> PortalDatabase {
        PortalDatabase::from_builtin_defaults(PathBuf::from("config/portals/portals.toml"))
    }

    #[test]
    fn builtin_defaults_load_without_error() {
        let d = db();
        assert!(!d.all().is_empty(), "expected non-empty portal list");
    }

    #[test]
    fn all_builtin_portals_have_non_empty_ids_and_zones() {
        for p in builtin_defaults().portals {
            assert!(!p.id.is_empty(), "portal id is empty");
            assert!(!p.destination_zone.is_empty(), "portal '{}'  has empty destination_zone", p.id);
        }
    }

    #[test]
    fn wizard_has_iceclad_at_level_52() {
        let d = db();
        let p = d.best_portal_to(CLASS_WIZARD, 52, "iceclad");
        assert!(p.is_some(), "expected Wizard iceclad portal at level 52");
        let p = p.unwrap();
        assert_eq!(p.class_id, CLASS_WIZARD);
        assert!(p.min_level <= 52);
    }

    #[test]
    fn druid_has_pine_twilight_iceclad_at_52() {
        let d = db();
        let p = d.best_portal_to(CLASS_DRUID, 52, "iceclad");
        assert!(p.is_some(), "expected Druid iceclad portal (Pine Twilight) at level 52");
        let p = p.unwrap();
        assert_eq!(p.id, "dru_port_iceclad");
    }

    #[test]
    fn druid_has_wandering_mind_wakening_at_54() {
        let d = db();
        let p = d.best_portal_to(CLASS_DRUID, 54, "wakening");
        assert!(p.is_some(), "expected Druid wakening portal (Wandering Mind)");
        let p = p.unwrap();
        assert_eq!(p.id, "dru_port_wakening");
    }

    #[test]
    fn druid_wakening_requires_level_54() {
        let d = db();
        // Level 53 Druid should NOT get wakening
        let p = d.best_portal_to(CLASS_DRUID, 53, "wakening");
        assert!(p.is_none(), "Druid wakening should require level 54");
    }

    #[test]
    fn wizard_iceclad_requires_level_52() {
        let d = db();
        let p = d.best_portal_to(CLASS_WIZARD, 51, "iceclad");
        assert!(p.is_none(), "Wizard iceclad should require level 52");
    }

    #[test]
    fn portals_for_class_returns_only_matching_class() {
        let d = db();
        for p in d.portals_for_class(CLASS_DRUID, 60) {
            assert_eq!(p.class_id, CLASS_DRUID, "non-Druid in Druid list: {}", p.id);
        }
    }

    #[test]
    fn portals_to_zone_returns_both_wiz_and_dru_for_iceclad() {
        let d = db();
        let portals = d.portals_to_zone("iceclad", 60);
        let classes: std::collections::HashSet<u8> = portals.iter().map(|p| p.class_id).collect();
        assert!(classes.contains(&CLASS_WIZARD), "expected Wizard iceclad");
        assert!(classes.contains(&CLASS_DRUID), "expected Druid iceclad");
    }

    #[test]
    fn safe_camp_for_iceclad_returns_coords() {
        let d = db();
        let camp = d.safe_camp_for("iceclad");
        assert!(camp.is_some(), "expected safe camp for iceclad");
    }

    #[test]
    fn safe_camp_lookup_is_case_insensitive() {
        let d = db();
        assert!(d.safe_camp_for("ICECLAD").is_some());
        assert!(d.safe_camp_for("IceClad").is_some());
    }

    #[test]
    fn best_portal_to_is_case_insensitive() {
        let d = db();
        assert!(d.best_portal_to(CLASS_WIZARD, 60, "ICECLAD").is_some());
        assert!(d.best_portal_to(CLASS_DRUID, 60, "Iceclad").is_some());
    }

    #[test]
    fn save_and_reload_roundtrip() {
        let d = db();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("portals.toml");
        let mut save_db = PortalDatabase::from_builtin_defaults(&path);
        // Manually set portals to match builtin
        save_db.portals = d.portals.clone();
        save_db.safe_camps = d.safe_camps.clone();
        save_db.save().expect("save failed");

        let loaded = PortalDatabase::load(&path).expect("reload failed");
        assert_eq!(loaded.all().len(), d.all().len());
        assert_eq!(loaded.safe_camps().len(), d.safe_camps().len());
    }

    #[test]
    fn validate_rejects_empty_id() {
        let portals = vec![Portal {
            id: "".into(),
            name: "Bad".into(),
            class_id: CLASS_WIZARD,
            min_level: 1,
            destination_zone: "commons".into(),
            landing_coords: None,
            kind: PortalKind::WizardPort,
            cooldown_secs: 0,
            group_port: false,
            spell_name: None,
            spell_id: None,
        }];
        assert!(PortalDatabase::validate(&portals).is_err());
    }

    #[test]
    fn validate_rejects_empty_destination_zone() {
        let portals = vec![Portal {
            id: "test".into(),
            name: "Bad".into(),
            class_id: CLASS_WIZARD,
            min_level: 1,
            destination_zone: "".into(),
            landing_coords: None,
            kind: PortalKind::WizardPort,
            cooldown_secs: 0,
            group_port: false,
            spell_name: None,
            spell_id: None,
        }];
        assert!(PortalDatabase::validate(&portals).is_err());
    }
}

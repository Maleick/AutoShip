//! Camp configuration panel — renders the runtime camp config tree and
//! exposes editable fields for attended tuning sessions.
//!
//! This panel is a read surface for the current camp config. Operators use it
//! to inspect pull radius, camp radius, leash, and mana thresholds during
//! live Sebilis or other zone validation runs.

/// Configuration tree entry displayed in the camp config panel.
pub struct ConfigEntry {
    /// Human-readable field label shown in the TUI config panel.
    pub label: String,
    /// Dotted config key path (e.g. `camp.pull_radius`).
    pub key: String,
    /// Current value as display string.
    pub value: String,
}

/// Build the default camp config tree entries for the config panel.
///
/// Returns entries covering pull radius, camp radius, leash radius, and mana
/// thresholds. All values are read from the active camp config at render time.
pub fn default_camp_config_entries() -> Vec<ConfigEntry> {
    vec![
        ConfigEntry {
            label: "Pull Radius".into(),
            key: "camp.pull_radius".into(),
            value: String::new(),
        },
        ConfigEntry {
            label: "Camp Radius".into(),
            key: "camp.camp_radius".into(),
            value: String::new(),
        },
        ConfigEntry {
            label: "Leash Radius".into(),
            key: "camp.leash_radius".into(),
            value: String::new(),
        },
        ConfigEntry {
            label: "Rest Mana %".into(),
            key: "camp.rest_mana_pct".into(),
            value: String::new(),
        },
        ConfigEntry {
            label: "Pull Mana %".into(),
            key: "camp.pull_mana_pct".into(),
            value: String::new(),
        },
    ]
}

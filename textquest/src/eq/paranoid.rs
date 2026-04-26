//! Paranoid — MQ2Paranoid parity.
//!
//! Monitors zone spawn lists and emits events when player characters zone in
//! or zone out. Configurable filter: all PCs, strangers only, or friends only.
//!
//! # Usage
//!
//! ```
//! use textquest::eq::paranoid::ParanoidMonitor;
//! use textquest::eq::structs::SpawnInfo;
//! use textquest_common::safety_features::ParanoidConfig;
//!
//! let mut monitor = ParanoidMonitor::new(ParanoidConfig::default());
//! let spawns: Vec<SpawnInfo> = Vec::new();
//! monitor.update_spawns(&spawns, "qeynos");
//!
//! for event in monitor.pending_events() {
//!     // Handle ZonedIn / ZonedOut events
//! }
//! ```

use std::collections::{HashMap, VecDeque};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use textquest_common::safety_features::{ParanoidConfig, ParanoidFilter};

use crate::eq::structs::{SpawnInfo, SpawnType};

/// Type of zone transition event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneTransitionType {
    /// Player character entered the zone.
    ZonedIn,
    /// Player character left the zone.
    ZonedOut,
}

impl ZoneTransitionType {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::ZonedIn => "zoned in",
            Self::ZonedOut => "zoned out",
        }
    }
}

/// A PC zone-in or zone-out event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParanoidEvent {
    /// Player name.
    pub player_name: String,
    /// Whether this player is on the trust (friends) list.
    pub is_friend: bool,
    /// Type of transition.
    pub transition: ZoneTransitionType,
    /// Zone where the transition occurred.
    pub zone: String,
    /// When the event was generated.
    pub timestamp: SystemTime,
}

/// Paranoid monitor — tracks PC zone transitions and emits filtered events.
pub struct ParanoidMonitor {
    config: ParanoidConfig,
    /// spawn_id → player_name for all PCs currently known in zone.
    known_pcs: HashMap<u32, String>,
    events: VecDeque<ParanoidEvent>,
}

impl ParanoidMonitor {
    /// Create a new monitor with the given configuration.
    #[must_use]
    pub fn new(config: ParanoidConfig) -> Self {
        Self {
            config,
            known_pcs: HashMap::new(),
            events: VecDeque::new(),
        }
    }

    /// Update configuration at runtime.
    pub fn set_config(&mut self, config: ParanoidConfig) {
        self.config = config;
    }

    /// Current configuration.
    #[must_use]
    pub fn config(&self) -> &ParanoidConfig {
        &self.config
    }

    /// Update with the current spawn list.
    ///
    /// Pass `own_name` to exclude the local character from zone-transition
    /// tracking. Pass `None` to track all PCs including self.
    pub fn update_spawns(&mut self, spawns: &[SpawnInfo], zone: &str, own_name: Option<&str>) {
        if !self.config.enabled {
            return;
        }

        let mut current_pc_ids: HashMap<u32, String> = HashMap::new();

        for spawn in spawns {
            if spawn.spawn_type != SpawnType::Player {
                continue;
            }
            if spawn.is_gm {
                continue;
            }
            if let Some(own) = own_name {
                if spawn.name.eq_ignore_ascii_case(own) {
                    continue;
                }
            }
            current_pc_ids.insert(spawn.spawn_id, spawn.name.clone());
        }

        // Detect zone-ins (new IDs not in known_pcs)
        for (id, name) in &current_pc_ids {
            if !self.known_pcs.contains_key(id) {
                self.maybe_emit(name, ZoneTransitionType::ZonedIn, zone);
            }
        }

        // Detect zone-outs (known IDs not in current list)
        let departed: Vec<(u32, String)> = self
            .known_pcs
            .iter()
            .filter(|(id, _)| !current_pc_ids.contains_key(id))
            .map(|(id, name)| (*id, name.clone()))
            .collect();

        for (_, name) in departed {
            self.maybe_emit(&name, ZoneTransitionType::ZonedOut, zone);
        }

        self.known_pcs = current_pc_ids;
    }

    fn maybe_emit(&mut self, player_name: &str, transition: ZoneTransitionType, zone: &str) {
        let is_friend = self.config.trust_list.contains(player_name);

        let should_emit = match self.config.filter {
            ParanoidFilter::All => true,
            ParanoidFilter::Strangers => !is_friend,
            ParanoidFilter::Friends => is_friend,
        };

        if !should_emit {
            return;
        }

        tracing::info!(
            player = %player_name,
            ?transition,
            zone = %zone,
            is_friend,
            "Paranoid zone transition"
        );

        self.events.push_back(ParanoidEvent {
            player_name: player_name.to_string(),
            is_friend,
            transition,
            zone: zone.to_string(),
            timestamp: SystemTime::now(),
        });
    }

    /// Drain and return all pending zone-transition events.
    #[must_use]
    pub fn pending_events(&mut self) -> Vec<ParanoidEvent> {
        self.events.drain(..).collect()
    }

    /// Number of PCs currently tracked in zone.
    #[must_use]
    pub fn pc_count(&self) -> usize {
        self.known_pcs.len()
    }

    /// Number of pending events.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.events.len()
    }

    /// Reset tracked state (e.g. on zone change).
    pub fn reset(&mut self) {
        self.known_pcs.clear();
        self.events.clear();
    }
}

impl Default for ParanoidMonitor {
    fn default() -> Self {
        Self::new(ParanoidConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eq::structs::{SpawnType, StandState};
    use textquest_common::safety_features::{ParanoidConfig, ParanoidFilter};
    use textquest_common::trust_list::TrustList;

    fn make_pc(id: u32, name: &str) -> SpawnInfo {
        SpawnInfo {
            name: name.to_string(),
            displayed_name: name.to_string(),
            lastname: String::new(),
            spawn_id: id,
            spawn_type: SpawnType::Player,
            level: 60,
            class_id: 1,
            class: None,
            stand_state: StandState::Standing,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 1000,
            hp_max: 1000,
            mana_current: 500,
            mana_max: 500,
            endurance_current: 100,
            endurance_max: 100,
            is_gm: false,
            race_id: 1,
            buff_slots: Vec::new(),
            spellbook: Vec::new(),
            memorized_spells: Vec::new(),
            cast_state: None,
        }
    }

    #[test]
    fn detects_zone_in() {
        let mut monitor = ParanoidMonitor::new(ParanoidConfig {
            filter: ParanoidFilter::All,
            ..ParanoidConfig::default()
        });
        let spawns = vec![make_pc(1, "Stranger")];
        monitor.update_spawns(&spawns, "qeynos", None);
        let events = monitor.pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].transition, ZoneTransitionType::ZonedIn);
        assert_eq!(events[0].player_name, "Stranger");
    }

    #[test]
    fn detects_zone_out() {
        let mut monitor = ParanoidMonitor::new(ParanoidConfig {
            filter: ParanoidFilter::All,
            ..ParanoidConfig::default()
        });
        let spawns = vec![make_pc(1, "Stranger")];
        monitor.update_spawns(&spawns, "qeynos", None);
        monitor.pending_events();

        let empty: Vec<SpawnInfo> = vec![];
        monitor.update_spawns(&empty, "qeynos", None);
        let events = monitor.pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].transition, ZoneTransitionType::ZonedOut);
    }

    #[test]
    fn strangers_filter_skips_friends() {
        let mut config = ParanoidConfig {
            filter: ParanoidFilter::Strangers,
            ..ParanoidConfig::default()
        };
        config.trust_list = TrustList::new(vec!["Healer".into()]);
        let mut monitor = ParanoidMonitor::new(config);

        let spawns = vec![make_pc(1, "Healer"), make_pc(2, "Outsider")];
        monitor.update_spawns(&spawns, "qeynos", None);
        let events = monitor.pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].player_name, "Outsider");
    }

    #[test]
    fn friends_filter_skips_strangers() {
        let mut config = ParanoidConfig {
            filter: ParanoidFilter::Friends,
            ..ParanoidConfig::default()
        };
        config.trust_list = TrustList::new(vec!["Healer".into()]);
        let mut monitor = ParanoidMonitor::new(config);

        let spawns = vec![make_pc(1, "Healer"), make_pc(2, "Outsider")];
        monitor.update_spawns(&spawns, "qeynos", None);
        let events = monitor.pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].player_name, "Healer");
    }

    #[test]
    fn own_character_excluded() {
        let mut monitor = ParanoidMonitor::new(ParanoidConfig {
            filter: ParanoidFilter::All,
            ..ParanoidConfig::default()
        });
        let spawns = vec![make_pc(1, "MyChar"), make_pc(2, "Other")];
        monitor.update_spawns(&spawns, "qeynos", Some("MyChar"));
        let events = monitor.pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].player_name, "Other");
    }

    #[test]
    fn disabled_monitor_emits_nothing() {
        let mut monitor = ParanoidMonitor::new(ParanoidConfig {
            enabled: false,
            ..ParanoidConfig::default()
        });
        let spawns = vec![make_pc(1, "Anyone")];
        monitor.update_spawns(&spawns, "qeynos", None);
        assert!(monitor.pending_events().is_empty());
    }

    #[test]
    fn stable_list_emits_no_events() {
        let mut monitor = ParanoidMonitor::new(ParanoidConfig {
            filter: ParanoidFilter::All,
            ..ParanoidConfig::default()
        });
        let spawns = vec![make_pc(1, "Player")];
        monitor.update_spawns(&spawns, "qeynos", None);
        monitor.pending_events();
        // Same list again — no new events
        monitor.update_spawns(&spawns, "qeynos", None);
        assert!(monitor.pending_events().is_empty());
    }
}

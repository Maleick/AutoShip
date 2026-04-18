use crate::types::{ClientId, GameState, SpawnData};

/// Runtime snapshot persisted for the web spawn finder dashboard.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LiveSpawnSnapshot {
    /// Per-client spawn observer snapshots.
    #[serde(default)]
    pub observers: Vec<LiveSpawnObserver>,
}

/// Spawn observer state for a single live client.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LiveSpawnObserver {
    /// Owning EQ process / client ID.
    pub client_id: ClientId,
    /// Character display name for the observing client.
    pub character_name: String,
    /// Current zone short name.
    pub zone_short_name: String,
    /// Current zone long name.
    pub zone_long_name: String,
    /// Local player snapshot used for distance calculations.
    pub local_player: SpawnData,
    /// Spawn ID of the observer's current target, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_spawn_id: Option<u32>,
    /// Nearby spawns visible to this observer.
    #[serde(default)]
    pub nearby_spawns: Vec<SpawnData>,
}

impl LiveSpawnObserver {
    /// Build a dashboard observer snapshot from a live `GameState`.
    #[must_use]
    pub fn from_game_state(fallback_name: Option<&str>, state: &GameState) -> Option<Self> {
        let local_player = state.local_player.clone()?;
        let character_name = if !local_player.displayed_name.trim().is_empty() {
            local_player.displayed_name.clone()
        } else if !local_player.name.trim().is_empty() {
            local_player.name.clone()
        } else {
            fallback_name.unwrap_or("Unknown").to_string()
        };

        Some(Self {
            client_id: state.client_id,
            character_name,
            zone_short_name: state.zone_short_name.clone(),
            zone_long_name: state.zone_long_name.clone(),
            local_player,
            target_spawn_id: state.target.as_ref().map(|target| target.spawn_id),
            nearby_spawns: state.nearby_spawns.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spawn(spawn_id: u32, name: &str) -> SpawnData {
        SpawnData {
            spawn_id,
            name: name.to_string(),
            displayed_name: name.to_string(),
            spawn_type: 1,
            level: 60,
            class_id: 1,
            race_id: 1,
            x: 10.0,
            y: 20.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 100,
            hp_max: 100,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
        }
    }

    #[test]
    fn live_spawn_observer_uses_local_player_name_when_available() {
        let state = GameState {
            client_id: 42,
            local_player: Some(make_spawn(10, "Frostreaver")),
            target: Some(make_spawn(900, "a frost giant")),
            nearby_spawns: vec![make_spawn(900, "a frost giant")],
            timestamp_ms: 123,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "kael".into(),
            zone_long_name: "Kael Drakkel".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let observer =
            LiveSpawnObserver::from_game_state(Some("Fallback"), &state).expect("observer");

        assert_eq!(observer.character_name, "Frostreaver");
        assert_eq!(observer.target_spawn_id, Some(900));
        assert_eq!(observer.nearby_spawns.len(), 1);
    }

    #[test]
    fn from_game_state_returns_none_when_no_local_player() {
        let state = GameState {
            client_id: 1,
            local_player: None,
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "nexus".into(),
            zone_long_name: "The Nexus".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        assert!(LiveSpawnObserver::from_game_state(Some("Fallback"), &state).is_none());
    }

    #[test]
    fn from_game_state_falls_back_to_name_when_displayed_name_empty() {
        let mut spawn = make_spawn(5, "Iceclaw");
        spawn.displayed_name = "  ".into();
        let state = GameState {
            client_id: 2,
            local_player: Some(spawn),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "velke".into(),
            zone_long_name: "Velketor's Labyrinth".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let observer = LiveSpawnObserver::from_game_state(Some("Fallback"), &state).expect("observer");
        assert_eq!(observer.character_name, "Iceclaw");
    }

    #[test]
    fn from_game_state_uses_provided_fallback_when_both_names_empty() {
        let mut spawn = make_spawn(6, "");
        spawn.displayed_name = "".into();
        let state = GameState {
            client_id: 3,
            local_player: Some(spawn),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "nexus".into(),
            zone_long_name: "The Nexus".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let observer =
            LiveSpawnObserver::from_game_state(Some("MyFallback"), &state).expect("observer");
        assert_eq!(observer.character_name, "MyFallback");
    }

    #[test]
    fn from_game_state_uses_unknown_when_both_names_empty_and_no_fallback() {
        let mut spawn = make_spawn(7, "");
        spawn.displayed_name = "".into();
        let state = GameState {
            client_id: 4,
            local_player: Some(spawn),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "nexus".into(),
            zone_long_name: "The Nexus".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let observer = LiveSpawnObserver::from_game_state(None, &state).expect("observer");
        assert_eq!(observer.character_name, "Unknown");
    }

    #[test]
    fn from_game_state_no_target_gives_none_target_spawn_id() {
        let state = GameState {
            client_id: 5,
            local_player: Some(make_spawn(10, "Ranger")),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "gfay".into(),
            zone_long_name: "Greater Faydark".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let observer = LiveSpawnObserver::from_game_state(None, &state).expect("observer");
        assert_eq!(observer.target_spawn_id, None);
        assert!(observer.nearby_spawns.is_empty());
    }

    #[test]
    fn from_game_state_populates_zone_and_client_id() {
        let state = GameState {
            client_id: 99,
            local_player: Some(make_spawn(1, "Paladin")),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "qeynos".into(),
            zone_long_name: "South Qeynos".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let observer = LiveSpawnObserver::from_game_state(None, &state).expect("observer");
        assert_eq!(observer.client_id, 99);
        assert_eq!(observer.zone_short_name, "qeynos");
        assert_eq!(observer.zone_long_name, "South Qeynos");
    }
}

use crate::{
    combat::{BuffCategory, BuffInfo, CombatStatus},
    types::{ClientId, GameState, PetData},
};

/// Environment toggle for exposing extended NetBots-style buff details.
pub const EXTENDED_STATE_ENV: &str = "TEXTQUEST_NETBOTS_EXTENDED";

/// Cross-client target summary shared between orchestrator, DLL, and web
/// surfaces.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SharedTargetState {
    /// EQ spawn ID of the current target.
    pub spawn_id: u32,
    /// Display name of the target.
    pub name: String,
    /// Current target HP percent.
    pub hp_pct: f32,
}

/// Buff summary shared across clients.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SharedBuffState {
    /// EQ spell ID for the active buff.
    pub spell_id: i32,
    /// Remaining duration in ticks when extended state sharing is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ticks: Option<i32>,
    /// Slot category (long/short buff) when extended state sharing is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<BuffCategory>,
}

impl SharedBuffState {
    #[must_use]
    pub fn from_buff_info(buff: &BuffInfo, include_extended: bool) -> Self {
        Self {
            spell_id: buff.spell_id,
            duration_ticks: include_extended.then_some(buff.duration_ticks),
            category: include_extended.then_some(buff.category),
        }
    }
}

/// Shared summary of a player's pet.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SharedPetState {
    /// EQ spawn ID of the pet.
    pub spawn_id: u32,
    /// Display name of the pet.
    pub name: String,
    /// Current target spawn ID for the pet, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<u32>,
    /// Current target display name for the pet, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
    /// Buff summary for the pet. This is populated only when extended sharing
    /// is enabled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buffs: Vec<SharedBuffState>,
}

impl SharedPetState {
    #[must_use]
    pub fn from_pet_data(pet: &PetData, include_extended: bool) -> Self {
        Self {
            spawn_id: pet.spawn_id,
            name: pet.name.clone(),
            target_id: pet.target_id,
            target_name: pet.target_name.clone(),
            buffs: pet
                .buffs
                .iter()
                .map(|buff| SharedBuffState::from_buff_info(buff, include_extended))
                .collect(),
        }
    }
}

/// NetBots-style cross-client state summary.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SharedClientState {
    /// Owning EQ process / client ID.
    pub client_id: ClientId,
    /// Character spawn ID, if the client is in-world.
    pub spawn_id: u32,
    /// Character display name.
    pub character_name: String,
    /// EQ class ID.
    pub class_id: u8,
    /// Character level.
    pub level: u8,
    /// Zone short name, for routing and lightweight dashboards.
    pub zone_short_name: String,
    /// Human-readable zone name.
    pub zone_long_name: String,
    /// Current HP percentage.
    pub hp_pct: f32,
    /// Current mana percentage.
    pub mana_pct: f32,
    /// Current endurance percentage.
    pub endurance_pct: f32,
    /// Whether the character is dead.
    pub is_dead: bool,
    /// Lightweight status label for dashboards.
    pub status: String,
    /// Current target summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<SharedTargetState>,
    /// Active buff summary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buffs: Vec<SharedBuffState>,
    /// Pet summary, if the character has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pet: Option<SharedPetState>,
}

impl SharedClientState {
    /// Build a shared roster entry from a live per-client `GameState`.
    #[must_use]
    pub fn from_game_state(
        fallback_name: Option<&str>,
        state: &GameState,
        include_extended: bool,
    ) -> Option<Self> {
        let player = state.local_player.as_ref()?;
        let character_name = if !player.displayed_name.trim().is_empty() {
            player.displayed_name.clone()
        } else if !player.name.trim().is_empty() {
            player.name.clone()
        } else {
            fallback_name.unwrap_or("Unknown").to_string()
        };

        Some(Self {
            client_id: state.client_id,
            spawn_id: player.spawn_id,
            character_name,
            class_id: player.class_id,
            level: player.level,
            zone_short_name: state.zone_short_name.clone(),
            zone_long_name: state.zone_long_name.clone(),
            hp_pct: player.hp_pct(),
            mana_pct: player.mana_pct(),
            endurance_pct: player.endurance_pct(),
            is_dead: matches!(state.combat_status, CombatStatus::Dead) || player.hp_current <= 0,
            status: combat_status_label(state.combat_status).to_string(),
            target: state.target.as_ref().map(|target| SharedTargetState {
                spawn_id: target.spawn_id,
                name: target.displayed_name.clone(),
                hp_pct: target.hp_pct(),
            }),
            buffs: state
                .active_buffs
                .iter()
                .map(|buff| SharedBuffState::from_buff_info(buff, include_extended))
                .collect(),
            pet: state
                .pet
                .as_ref()
                .map(|pet| SharedPetState::from_pet_data(pet, include_extended)),
        })
    }
}

/// Human-readable status label for cross-client dashboards.
#[must_use]
pub fn combat_status_label(status: CombatStatus) -> &'static str {
    match status {
        CombatStatus::Idle | CombatStatus::Recovering => "idle",
        CombatStatus::Dead => "dead",
        CombatStatus::Pulling { .. } => "pulling",
        CombatStatus::Engaging { .. } | CombatStatus::Casting { .. } | CombatStatus::OnGcd => {
            "active"
        }
        CombatStatus::Fleeing => "fleeing",
    }
}

/// Whether extended cross-client state sharing is enabled for this process.
#[must_use]
pub fn extended_state_enabled() -> bool {
    std::env::var(EXTENDED_STATE_ENV)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{combat::CombatStatus, nav::NavStatus, types::SpawnData};

    fn make_spawn() -> SpawnData {
        SpawnData {
            spawn_id: 7,
            name: "Frostreaver".into(),
            displayed_name: "Frostreaver".into(),
            spawn_type: 0,
            level: 60,
            class_id: 2,
            race_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 750,
            hp_max: 1000,
            mana_current: 600,
            mana_max: 1000,
            endurance_current: 250,
            endurance_max: 500,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
        }
    }

    #[test]
    fn shared_client_state_uses_extended_buff_details_when_requested() {
        let state = GameState {
            client_id: 77,
            local_player: Some(make_spawn()),
            target: Some(SpawnData {
                spawn_id: 88,
                name: "a frost giant".into(),
                displayed_name: "a frost giant".into(),
                spawn_type: 1,
                level: 60,
                class_id: 1,
                race_id: 9,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                heading: 0.0,
                hp_current: 500,
                hp_max: 1000,
                mana_current: 0,
                mana_max: 0,
                endurance_current: 0,
                endurance_max: 0,
                speed_run: 0.0,
                stand_state: 0,
                is_gm: false,
            }),
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Casting {
                spell_slot: 1,
                target_id: 88,
            },
            zone_short_name: "kael".into(),
            zone_long_name: "Kael Drakkel".into(),
            active_buffs: vec![BuffInfo {
                spell_id: 1234,
                duration_ticks: 12,
                initial_duration: 20,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 60,
                slot_index: 4,
            }],
            pet: Some(PetData {
                spawn_id: 99,
                name: "a warder".into(),
                target_id: Some(88),
                target_name: Some("a frost giant".into()),
                buffs: vec![BuffInfo {
                    spell_id: 2222,
                    duration_ticks: 8,
                    initial_duration: 15,
                    hit_count: 0,
                    category: BuffCategory::ShortBuff,
                    caster_level: 60,
                    slot_index: 1,
                }],
            }),
            actual_version: None,
        };

        let shared =
            SharedClientState::from_game_state(None, &state, true).expect("shared state exists");

        assert_eq!(shared.character_name, "Frostreaver");
        assert_eq!(
            shared.target.as_ref().map(|target| target.spawn_id),
            Some(88)
        );
        assert_eq!(
            shared.target.as_ref().map(|target| target.hp_pct),
            Some(50.0)
        );
        assert_eq!(shared.buffs[0].duration_ticks, Some(12));
        assert_eq!(shared.pet.as_ref().map(|pet| pet.buffs.len()), Some(1));
        assert_eq!(shared.status, "active");
        assert!((shared.endurance_pct - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn shared_client_state_omits_extended_buff_details_when_disabled() {
        let state = GameState {
            client_id: 77,
            local_player: Some(make_spawn()),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "kael".into(),
            zone_long_name: "Kael Drakkel".into(),
            active_buffs: vec![BuffInfo {
                spell_id: 1234,
                duration_ticks: 12,
                initial_duration: 20,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 60,
                slot_index: 4,
            }],
            pet: Some(PetData {
                spawn_id: 99,
                name: "a warder".into(),
                target_id: None,
                target_name: None,
                buffs: vec![BuffInfo {
                    spell_id: 2222,
                    duration_ticks: 8,
                    initial_duration: 15,
                    hit_count: 0,
                    category: BuffCategory::ShortBuff,
                    caster_level: 60,
                    slot_index: 1,
                }],
            }),
            actual_version: None,
        };

        let shared =
            SharedClientState::from_game_state(None, &state, false).expect("shared state exists");

        assert_eq!(shared.buffs[0].spell_id, 1234);
        assert_eq!(shared.buffs[0].duration_ticks, None);
        assert_eq!(
            shared.pet.as_ref().map(|pet| pet.buffs[0].duration_ticks),
            Some(None)
        );
    }

    #[test]
    fn from_game_state_returns_none_when_no_local_player() {
        let state = GameState {
            client_id: 1,
            local_player: None,
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "nexus".into(),
            zone_long_name: "The Nexus".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        assert!(SharedClientState::from_game_state(None, &state, false).is_none());
    }

    #[test]
    fn from_game_state_falls_back_to_name_when_displayed_name_whitespace() {
        let mut spawn = make_spawn();
        spawn.displayed_name = "   ".into();
        spawn.name = "Iceclaw".into();
        let state = GameState {
            client_id: 2,
            local_player: Some(spawn),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "velke".into(),
            zone_long_name: "Velketor's Labyrinth".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let shared = SharedClientState::from_game_state(None, &state, false).expect("state");
        assert_eq!(shared.character_name, "Iceclaw");
    }

    #[test]
    fn from_game_state_uses_fallback_name_when_both_names_empty() {
        let mut spawn = make_spawn();
        spawn.displayed_name = "".into();
        spawn.name = "".into();
        let state = GameState {
            client_id: 3,
            local_player: Some(spawn),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "nexus".into(),
            zone_long_name: "The Nexus".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let shared =
            SharedClientState::from_game_state(Some("MyFallback"), &state, false).expect("state");
        assert_eq!(shared.character_name, "MyFallback");
    }

    #[test]
    fn from_game_state_uses_unknown_when_both_names_empty_and_no_fallback() {
        let mut spawn = make_spawn();
        spawn.displayed_name = "".into();
        spawn.name = "".into();
        let state = GameState {
            client_id: 4,
            local_player: Some(spawn),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "nexus".into(),
            zone_long_name: "The Nexus".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let shared = SharedClientState::from_game_state(None, &state, false).expect("state");
        assert_eq!(shared.character_name, "Unknown");
    }

    #[test]
    fn is_dead_when_hp_current_is_zero() {
        let mut spawn = make_spawn();
        spawn.hp_current = 0;
        let state = GameState {
            client_id: 5,
            local_player: Some(spawn),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "nexus".into(),
            zone_long_name: "The Nexus".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let shared = SharedClientState::from_game_state(None, &state, false).expect("state");
        assert!(shared.is_dead);
    }

    #[test]
    fn is_dead_when_combat_status_dead() {
        let state = GameState {
            client_id: 6,
            local_player: Some(make_spawn()),
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Dead,
            zone_short_name: "nexus".into(),
            zone_long_name: "The Nexus".into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };

        let shared = SharedClientState::from_game_state(None, &state, false).expect("state");
        assert!(shared.is_dead);
        assert_eq!(shared.status, "dead");
    }

    #[test]
    fn combat_status_label_covers_all_variants() {
        assert_eq!(combat_status_label(CombatStatus::Idle), "idle");
        assert_eq!(combat_status_label(CombatStatus::Recovering), "idle");
        assert_eq!(combat_status_label(CombatStatus::Dead), "dead");
        assert_eq!(
            combat_status_label(CombatStatus::Pulling { target_id: 1 }),
            "pulling"
        );
        assert_eq!(
            combat_status_label(CombatStatus::Engaging { target_id: 1 }),
            "active"
        );
        assert_eq!(
            combat_status_label(CombatStatus::Casting {
                spell_slot: 0,
                target_id: 1
            }),
            "active"
        );
        assert_eq!(combat_status_label(CombatStatus::OnGcd), "active");
        assert_eq!(combat_status_label(CombatStatus::Fleeing), "fleeing");
    }

    #[test]
    fn shared_buff_state_from_buff_info_with_extended() {
        let buff = BuffInfo {
            spell_id: 5678,
            duration_ticks: 20,
            initial_duration: 30,
            hit_count: 0,
            category: BuffCategory::ShortBuff,
            caster_level: 55,
            slot_index: 2,
        };

        let state = SharedBuffState::from_buff_info(&buff, true);
        assert_eq!(state.spell_id, 5678);
        assert_eq!(state.duration_ticks, Some(20));
        assert_eq!(state.category, Some(BuffCategory::ShortBuff));
    }

    #[test]
    fn shared_buff_state_from_buff_info_without_extended() {
        let buff = BuffInfo {
            spell_id: 5678,
            duration_ticks: 20,
            initial_duration: 30,
            hit_count: 0,
            category: BuffCategory::ShortBuff,
            caster_level: 55,
            slot_index: 2,
        };

        let state = SharedBuffState::from_buff_info(&buff, false);
        assert_eq!(state.spell_id, 5678);
        assert_eq!(state.duration_ticks, None);
        assert_eq!(state.category, None);
    }

    #[test]
    fn shared_pet_state_from_pet_data_with_extended_buffs() {
        let pet = PetData {
            spawn_id: 42,
            name: "Fido".into(),
            target_id: Some(10),
            target_name: Some("an orc".into()),
            buffs: vec![BuffInfo {
                spell_id: 9999,
                duration_ticks: 5,
                initial_duration: 10,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 60,
                slot_index: 0,
            }],
        };

        let state = SharedPetState::from_pet_data(&pet, true);
        assert_eq!(state.spawn_id, 42);
        assert_eq!(state.name, "Fido");
        assert_eq!(state.target_id, Some(10));
        assert_eq!(state.buffs.len(), 1);
        assert_eq!(state.buffs[0].duration_ticks, Some(5));
    }

    #[test]
    fn shared_pet_state_from_pet_data_no_target_no_extended() {
        let pet = PetData {
            spawn_id: 11,
            name: "Rex".into(),
            target_id: None,
            target_name: None,
            buffs: Vec::new(),
        };

        let state = SharedPetState::from_pet_data(&pet, false);
        assert_eq!(state.target_id, None);
        assert!(state.buffs.is_empty());
    }
}

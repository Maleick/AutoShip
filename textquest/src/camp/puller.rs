//! Pull target selection — picks the best mob to pull from nearby spawns.
//! Pull state machine — manages four pull modes (Normal, Chain, Hunt, Farm) with mode transition callbacks.

use crate::{
    camp::{cc::CcTracker, config::CampConfig, positioning::distance_2d},
    eq::named_tracker::NamedTracker,
};
use std::fmt;

/// Pull mode determines the pulling behavior and strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullMode {
    /// Standard pull — single pull at a time, wait for fight to finish before next pull.
    Normal,
    /// Continuous pull loop — keep pulling while group is fighting or recovering.
    Chain,
    /// Seek and pull named mobs — prioritize named/epic mobs when available.
    Hunt,
    /// Stationary camp farming — stay in one fixed position and pull nearby mobs.
    Farm,
}

impl fmt::Display for PullMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal => write!(f, "Normal"),
            Self::Chain => write!(f, "Chain"),
            Self::Hunt => write!(f, "Hunt"),
            Self::Farm => write!(f, "Farm"),
        }
    }
}

/// Callback function type for mode change events.
/// Called when the pull state machine transitions to a new mode.
pub type OnModeChangeCallback = Box<dyn Fn(PullMode, PullMode) + Send + Sync>;

/// Re-export `PullMode` from `textquest_common` for convenience.
pub use textquest_common::combat::PullMode;

/// FSM states for the pull loop — mirrors rgmercs 11-state machine (gap #2).
///
/// Transitions:
/// ```text
/// Idle → Searching → Moving → Pulling → Waiting → Fighting
///                                              ↘ Aborting → Returning → Idle
///                      ↑←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←↗
/// ```
/// `Paused` overlays any state and preserves the previous state for resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFsmState {
    /// No pull in progress; waiting for the fight to end or mana to recover.
    #[default]
    Idle,
    /// Scanning nearby spawns for a valid pull target.
    Searching,
    /// Navigating toward the selected pull target.
    Moving,
    /// Pulling — aggro spell/bow/taunt sent, waiting for mob to run back.
    Pulling,
    /// Mob is incoming; group is preparing (stepping back, CC setup, etc.).
    Waiting,
    /// Active fight underway.
    Fighting,
    /// Pull aborted (CC broke, mob fled, group wiped, etc.).
    Aborting,
    /// Puller returning to camp anchor after abort or fight end.
    Returning,
    /// Paused by operator command; resumes from the previous state on unpause.
    Paused,
    /// Chain-pull sub-state: pull accepted and puller is already heading for
    /// the next target while the group finishes the current mob.
    ChainScouting,
    /// Hunt sub-state: roaming the zone looking for a priority named target.
    HuntRoaming,
}

impl std::fmt::Display for PullFsmState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Searching => write!(f, "Searching"),
            Self::Moving => write!(f, "Moving"),
            Self::Pulling => write!(f, "Pulling"),
            Self::Waiting => write!(f, "Waiting"),
            Self::Fighting => write!(f, "Fighting"),
            Self::Aborting => write!(f, "Aborting"),
            Self::Returning => write!(f, "Returning"),
            Self::Paused => write!(f, "Paused"),
            Self::ChainScouting => write!(f, "ChainScouting"),
            Self::HuntRoaming => write!(f, "HuntRoaming"),
        }
    }
}

/// Spawn type discriminator matching EQ's internal spawn types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnType {
    /// A real player character.
    Player,
    /// A non-player character (mob).
    Npc,
    /// A player or NPC corpse.
    Corpse,
    /// Other spawn type (objects, auras, etc.).
    Other,
}

/// A nearby spawn visible to the puller.
#[derive(Debug, Clone)]
pub struct NearbySpawn {
    /// EQ spawn ID.
    pub spawn_id: u32,
    /// Display name of the spawn.
    pub name: String,
    /// Type of this spawn (player, NPC, corpse).
    pub spawn_type: SpawnType,
    /// X coordinate in EQ world units.
    pub x: f32,
    /// Y coordinate in EQ world units.
    pub y: f32,
    /// Z coordinate in EQ world units.
    pub z: f32,
}

/// Pull state machine managing mode transitions and callbacks.
pub struct PullStateMachine {
    /// Current pull mode.
    current_mode: PullMode,
    /// Optional callback fired on mode transitions.
    on_mode_change: Option<OnModeChangeCallback>,
}

impl PullStateMachine {
    /// Creates a new pull state machine starting in Normal mode.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_mode: PullMode::Normal,
            on_mode_change: None,
        }
    }

    /// Creates a pull state machine with an initial mode and callback.
    #[must_use]
    pub fn with_mode_and_callback(
        initial_mode: PullMode,
        callback: OnModeChangeCallback,
    ) -> Self {
        Self {
            current_mode: initial_mode,
            on_mode_change: Some(callback),
        }
    }

    /// Returns the current pull mode.
    #[must_use]
    pub fn current_mode(&self) -> PullMode {
        self.current_mode
    }

    /// Sets a new mode change callback.
    pub fn set_on_mode_change(&mut self, callback: OnModeChangeCallback) {
        self.on_mode_change = Some(callback);
    }

    /// Transitions to a new pull mode.
    /// Fires the OnModeChange callback if registered, passing the old and new modes.
    pub fn transition_to(&mut self, new_mode: PullMode) {
        if self.current_mode == new_mode {
            return; // No transition needed
        }

        let old_mode = self.current_mode;
        self.current_mode = new_mode;

        // Fire callback if registered
        if let Some(ref callback) = self.on_mode_change {
            callback(old_mode, new_mode);
        }
    }

    /// Returns true if currently in Chain mode (continuous pulling).
    #[must_use]
    pub fn is_chain_mode(&self) -> bool {
        self.current_mode == PullMode::Chain
    }

    /// Returns true if currently in Hunt mode (named mob focus).
    #[must_use]
    pub fn is_hunt_mode(&self) -> bool {
        self.current_mode == PullMode::Hunt
    }

    /// Returns true if currently in Farm mode (stationary camp).
    #[must_use]
    pub fn is_farm_mode(&self) -> bool {
        self.current_mode == PullMode::Farm
    }

    /// Returns true if currently in Normal mode (single pull).
    #[must_use]
    pub fn is_normal_mode(&self) -> bool {
        self.current_mode == PullMode::Normal
    }
}

impl Default for PullStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// Select the best pull target from nearby spawns.
///
/// Filters:
/// - Must be an NPC (not player, corpse, or other)
/// - Must be within `pull_radius` of the camp's `pull_point`
/// - Must not already be tracked by the CC system (already pulled/CC'd)
///
/// Preference order:
/// 1. Mobs matching `pull_mob_names` from camp config (if configured)
/// 2. Mobs on the HVT (high-value target) watchlist
/// 3. Closest NPC to the pull point
#[must_use]
pub fn select_pull_target(
    nearby_spawns: &[NearbySpawn],
    camp_config: &CampConfig,
    cc_tracker: &CcTracker,
    hvt_watchlist: &[String],
) -> Option<String> {
    let pull_x = camp_config.pull_point[0];
    let pull_y = camp_config.pull_point[1];

    // IDs already tracked by CC (already in camp, being CC'd or killed)
    let cc_ids: Vec<u32> = cc_tracker.targets.iter().map(|t| t.spawn_id).collect();

    // Filter to valid pull candidates
    let candidates: Vec<&NearbySpawn> = nearby_spawns
        .iter()
        .filter(|s| s.spawn_type == SpawnType::Npc)
        .filter(|s| distance_2d(s.x, s.y, pull_x, pull_y) <= camp_config.pull_radius)
        .filter(|s| !cc_ids.contains(&s.spawn_id))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // Prefer configured pull mob names
    if !camp_config.pull_mob_names.is_empty() {
        let config_match = candidates
            .iter()
            .filter(|s| camp_config.pull_mob_names.contains(&s.name))
            .min_by(|a, b| {
                let da = distance_2d(a.x, a.y, pull_x, pull_y);
                let db = distance_2d(b.x, b.y, pull_x, pull_y);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(target) = config_match {
            return Some(target.name.clone());
        }
    }

    // Prefer HVT watchlist targets
    if !hvt_watchlist.is_empty() {
        let hvt_match = candidates
            .iter()
            .filter(|s| hvt_watchlist.contains(&s.name))
            .min_by(|a, b| {
                let da = distance_2d(a.x, a.y, pull_x, pull_y);
                let db = distance_2d(b.x, b.y, pull_x, pull_y);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(target) = hvt_match {
            return Some(target.name.clone());
        }
    }

    // Fall back to closest NPC
    candidates
        .iter()
        .min_by(|a, b| {
            let da = distance_2d(a.x, a.y, pull_x, pull_y);
            let db = distance_2d(b.x, b.y, pull_x, pull_y);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|s| s.name.clone())
}

/// Extended pull target selection that checks the named tracker first.
/// If a named mob from the database is alive and in range, it takes priority
/// over all other targets.
#[must_use]
pub fn select_pull_target_with_named(
    nearby_spawns: &[NearbySpawn],
    camp_config: &CampConfig,
    cc_tracker: &CcTracker,
    hvt_watchlist: &[String],
    named_tracker: &NamedTracker,
) -> Option<String> {
    // Check for a high-priority named mob override
    if let Some(priority_named) = named_tracker.priority_target() {
        // Verify the named mob is actually in our spawn list and in range
        let pull_x = camp_config.pull_point[0];
        let pull_y = camp_config.pull_point[1];
        let cc_ids: Vec<u32> = cc_tracker.targets.iter().map(|t| t.spawn_id).collect();

        let in_range = nearby_spawns.iter().any(|s| {
            s.spawn_type == SpawnType::Npc
                && s.name == priority_named.name
                && distance_2d(s.x, s.y, pull_x, pull_y) <= camp_config.pull_radius
                && !cc_ids.contains(&s.spawn_id)
        });

        if in_range {
            return Some(priority_named.name.clone());
        }
    }

    // Fall back to normal selection
    select_pull_target(nearby_spawns, camp_config, cc_tracker, hvt_watchlist)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn test_config() -> CampConfig {
        CampConfig {
            name: "test_camp".into(),
            zone: "crushbone".into(),
            camp_center: [100.0, 200.0, 0.0],
            pull_point: [150.0, 250.0, 0.0],
            pull_radius: 200.0,
            camp_radius: 30.0,
            leash_radius: 100.0,
            rest_mana_pct: 60,
            pull_mana_pct: 30,
            level_range: [5, 12],
            pull_mob_names: vec!["an orc pawn".into()],
            ignore_mob_names: Vec::new(),
            burn_mob_names: Vec::new(),
            return_no_aggro: false,
            next_camp: None,
            prev_camp: None,
        }
    }

    fn make_spawn(id: u32, name: &str, spawn_type: SpawnType, x: f32, y: f32) -> NearbySpawn {
        NearbySpawn {
            spawn_id: id,
            name: name.into(),
            spawn_type,
            x,
            y,
            z: 0.0,
        }
    }

    #[test]
    fn test_selects_configured_mob_name() {
        let spawns = vec![
            make_spawn(1, "an orc centurion", SpawnType::Npc, 160.0, 260.0),
            make_spawn(2, "an orc pawn", SpawnType::Npc, 170.0, 270.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        assert_eq!(result, Some("an orc pawn".into()));
    }

    #[test]
    fn test_selects_hvt_when_no_config_match() {
        let mut config = test_config();
        config.pull_mob_names.clear();
        let spawns = vec![
            make_spawn(1, "an orc centurion", SpawnType::Npc, 160.0, 260.0),
            make_spawn(2, "a named mob", SpawnType::Npc, 170.0, 270.0),
        ];
        let cc = CcTracker::new();
        let hvt = vec!["a named mob".to_string()];
        let result = select_pull_target(&spawns, &config, &cc, &hvt);
        assert_eq!(result, Some("a named mob".into()));
    }

    #[test]
    fn test_selects_closest_fallback() {
        let mut config = test_config();
        config.pull_mob_names.clear();
        let spawns = vec![
            make_spawn(1, "far orc", SpawnType::Npc, 300.0, 400.0),
            make_spawn(2, "close orc", SpawnType::Npc, 155.0, 255.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &config, &cc, &[]);
        assert_eq!(result, Some("close orc".into()));
    }

    #[test]
    fn test_filters_out_players() {
        let spawns = vec![
            make_spawn(1, "PlayerChar", SpawnType::Player, 155.0, 255.0),
            make_spawn(2, "an orc pawn", SpawnType::Npc, 170.0, 270.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        assert_eq!(result, Some("an orc pawn".into()));
    }

    #[test]
    fn test_filters_out_corpses() {
        let spawns = vec![make_spawn(
            1,
            "an orc pawn",
            SpawnType::Corpse,
            155.0,
            255.0,
        )];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_filters_out_cc_tracked_mobs() {
        let spawns = vec![
            make_spawn(1, "an orc pawn", SpawnType::Npc, 155.0, 255.0),
            make_spawn(2, "an orc centurion", SpawnType::Npc, 160.0, 260.0),
        ];
        let mut cc = CcTracker::new();
        cc.update(&[(1, "an orc pawn".into())], None, 0);
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        // Spawn 1 is tracked by CC, so should pick spawn 2
        assert_eq!(result, Some("an orc centurion".into()));
    }

    #[test]
    fn test_filters_out_of_range() {
        let spawns = vec![make_spawn(1, "an orc pawn", SpawnType::Npc, 9999.0, 9999.0)];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_empty_spawns() {
        let cc = CcTracker::new();
        let result = select_pull_target(&[], &test_config(), &cc, &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_config_match_prefers_closest() {
        let config = CampConfig {
            pull_mob_names: vec!["an orc pawn".into()],
            ..test_config()
        };
        let spawns = vec![
            make_spawn(1, "an orc pawn", SpawnType::Npc, 300.0, 350.0),
            make_spawn(2, "an orc pawn", SpawnType::Npc, 155.0, 255.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &config, &cc, &[]);
        // Should pick the closer orc pawn (spawn 2)
        assert_eq!(result, Some("an orc pawn".into()));
    }

    #[test]
    fn test_filters_out_other_spawn_type() {
        let spawns = vec![make_spawn(1, "a trap", SpawnType::Other, 155.0, 255.0)];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_all_cc_tracked_returns_none() {
        let spawns = vec![make_spawn(1, "an orc pawn", SpawnType::Npc, 155.0, 255.0)];
        let mut cc = CcTracker::new();
        cc.update(&[(1, "an orc pawn".into())], None, 0);
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_hvt_prefers_closest_of_multiple_hvts() {
        let mut config = test_config();
        config.pull_mob_names.clear();
        let spawns = vec![
            make_spawn(1, "rare dragon", SpawnType::Npc, 300.0, 350.0),
            make_spawn(2, "rare dragon", SpawnType::Npc, 155.0, 255.0),
        ];
        let cc = CcTracker::new();
        let hvt = vec!["rare dragon".to_string()];
        let result = select_pull_target(&spawns, &config, &cc, &hvt);
        assert_eq!(result, Some("rare dragon".into()));
    }

    #[test]
    fn test_config_match_beats_hvt() {
        let config = test_config(); // pull_mob_names has "an orc pawn"
        let spawns = vec![
            make_spawn(1, "an orc pawn", SpawnType::Npc, 170.0, 270.0),
            make_spawn(2, "hvt_mob", SpawnType::Npc, 155.0, 255.0),
        ];
        let cc = CcTracker::new();
        let hvt = vec!["hvt_mob".to_string()];
        let result = select_pull_target(&spawns, &config, &cc, &hvt);
        // Config match takes priority over HVT
        assert_eq!(result, Some("an orc pawn".into()));
    }

    #[test]
    fn test_mixed_types_only_npc_selected() {
        let mut config = test_config();
        config.pull_mob_names.clear();
        let spawns = vec![
            make_spawn(1, "PlayerOne", SpawnType::Player, 155.0, 255.0),
            make_spawn(2, "orc_corpse", SpawnType::Corpse, 156.0, 256.0),
            make_spawn(3, "a_trap", SpawnType::Other, 157.0, 257.0),
            make_spawn(4, "an orc", SpawnType::Npc, 160.0, 260.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &config, &cc, &[]);
        assert_eq!(result, Some("an orc".into()));
    }

    #[test]
    fn test_spawn_type_equality() {
        assert_eq!(SpawnType::Player, SpawnType::Player);
        assert_eq!(SpawnType::Npc, SpawnType::Npc);
        assert_eq!(SpawnType::Corpse, SpawnType::Corpse);
        assert_eq!(SpawnType::Other, SpawnType::Other);
        assert_ne!(SpawnType::Player, SpawnType::Npc);
    }

    #[test]
    fn test_nearby_spawn_construction() {
        let s = make_spawn(42, "test mob", SpawnType::Npc, 1.0, 2.0);
        assert_eq!(s.spawn_id, 42);
        assert_eq!(s.name, "test mob");
        assert_eq!(s.x, 1.0);
        assert_eq!(s.y, 2.0);
        assert_eq!(s.z, 0.0);
    }

    #[test]
    fn test_select_pull_target_with_named_priority() {
        let mut config = test_config();
        config.pull_mob_names.clear();
        let spawns = vec![
            make_spawn(1, "common orc", SpawnType::Npc, 155.0, 255.0),
            make_spawn(2, "Emperor Crush", SpawnType::Npc, 160.0, 260.0),
        ];
        let cc = CcTracker::new();
        let named = NamedTracker::new();
        // Without a priority target in named tracker, falls back to normal logic
        let result = select_pull_target_with_named(&spawns, &config, &cc, &[], &named);
        assert_eq!(result, Some("common orc".into())); // closest
    }

    #[test]
    fn test_select_pull_target_with_named_no_spawns() {
        let config = test_config();
        let cc = CcTracker::new();
        let named = NamedTracker::new();
        let result = select_pull_target_with_named(&[], &config, &cc, &[], &named);
        assert_eq!(result, None);
    }

    #[test]
    fn test_boundary_pull_radius() {
        let mut config = test_config();
        config.pull_mob_names.clear();
        config.pull_radius = 10.0;
        // Spawn exactly at pull_radius distance
        let spawns = vec![
            make_spawn(1, "boundary orc", SpawnType::Npc, 160.0, 250.0), /* dist = 10.0 from
                                                                          * pull_point */
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &config, &cc, &[]);
        // distance_2d(160, 250, 150, 250) = 10.0, equal to pull_radius => should be
        // included
        assert_eq!(result, Some("boundary orc".into()));
    }

    // -- Pull mode and state machine tests --

    #[test]
    fn test_pull_mode_display() {
        assert_eq!(format!("{}", PullMode::Normal), "Normal");
        assert_eq!(format!("{}", PullMode::Chain), "Chain");
        assert_eq!(format!("{}", PullMode::Hunt), "Hunt");
        assert_eq!(format!("{}", PullMode::Farm), "Farm");
    }

    #[test]
    fn test_pull_mode_equality() {
        assert_eq!(PullMode::Normal, PullMode::Normal);
        assert_eq!(PullMode::Chain, PullMode::Chain);
        assert_ne!(PullMode::Normal, PullMode::Chain);
    }

    #[test]
    fn test_pull_state_machine_default() {
        let fsm = PullStateMachine::default();
        assert_eq!(fsm.current_mode(), PullMode::Normal);
    }

    #[test]
    fn test_pull_state_machine_new() {
        let fsm = PullStateMachine::new();
        assert_eq!(fsm.current_mode(), PullMode::Normal);
    }

    #[test]
    fn test_pull_state_machine_starts_normal() {
        let fsm = PullStateMachine::new();
        assert!(fsm.is_normal_mode());
        assert!(!fsm.is_chain_mode());
        assert!(!fsm.is_hunt_mode());
        assert!(!fsm.is_farm_mode());
    }

    #[test]
    fn test_pull_state_machine_transition_to_chain() {
        let mut fsm = PullStateMachine::new();
        fsm.transition_to(PullMode::Chain);
        assert_eq!(fsm.current_mode(), PullMode::Chain);
        assert!(fsm.is_chain_mode());
        assert!(!fsm.is_normal_mode());
    }

    #[test]
    fn test_pull_state_machine_transition_to_hunt() {
        let mut fsm = PullStateMachine::new();
        fsm.transition_to(PullMode::Hunt);
        assert_eq!(fsm.current_mode(), PullMode::Hunt);
        assert!(fsm.is_hunt_mode());
    }

    #[test]
    fn test_pull_state_machine_transition_to_farm() {
        let mut fsm = PullStateMachine::new();
        fsm.transition_to(PullMode::Farm);
        assert_eq!(fsm.current_mode(), PullMode::Farm);
        assert!(fsm.is_farm_mode());
    }

    #[test]
    fn test_pull_state_machine_no_op_transition() {
        let mut fsm = PullStateMachine::new();
        fsm.transition_to(PullMode::Normal); // Already in Normal
        assert_eq!(fsm.current_mode(), PullMode::Normal);
    }

    #[test]
    fn test_pull_state_machine_callback_fires_on_transition() {
        let callback_log = Arc::new(Mutex::new(Vec::new()));
        let log_clone = callback_log.clone();

        let callback = Box::new(move |from: PullMode, to: PullMode| {
            let mut log = log_clone.lock().unwrap();
            log.push((from, to));
        });

        let mut fsm = PullStateMachine::with_mode_and_callback(PullMode::Normal, callback);
        fsm.transition_to(PullMode::Chain);

        let log = callback_log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0], (PullMode::Normal, PullMode::Chain));
    }

    #[test]
    fn test_pull_state_machine_callback_no_fire_on_same_mode() {
        let callback_log = Arc::new(Mutex::new(Vec::new()));
        let log_clone = callback_log.clone();

        let callback = Box::new(move |_from: PullMode, _to: PullMode| {
            let mut log = log_clone.lock().unwrap();
            log.push(true);
        });

        let mut fsm = PullStateMachine::with_mode_and_callback(PullMode::Normal, callback);
        fsm.transition_to(PullMode::Normal); // No transition

        let log = callback_log.lock().unwrap();
        assert!(log.is_empty());
    }

    #[test]
    fn test_pull_state_machine_multiple_transitions() {
        let callback_log = Arc::new(Mutex::new(Vec::new()));
        let log_clone = callback_log.clone();

        let callback = Box::new(move |from: PullMode, to: PullMode| {
            let mut log = log_clone.lock().unwrap();
            log.push((from, to));
        });

        let mut fsm = PullStateMachine::with_mode_and_callback(PullMode::Normal, callback);
        fsm.transition_to(PullMode::Chain);
        fsm.transition_to(PullMode::Hunt);
        fsm.transition_to(PullMode::Farm);

        let log = callback_log.lock().unwrap();
        assert_eq!(log.len(), 3);
        assert_eq!(log[0], (PullMode::Normal, PullMode::Chain));
        assert_eq!(log[1], (PullMode::Chain, PullMode::Hunt));
        assert_eq!(log[2], (PullMode::Hunt, PullMode::Farm));
    }

    #[test]
    fn test_pull_state_machine_set_callback() {
        let callback_log = Arc::new(Mutex::new(Vec::new()));
        let log_clone = callback_log.clone();

        let callback = Box::new(move |from: PullMode, to: PullMode| {
            let mut log = log_clone.lock().unwrap();
            log.push((from, to));
        });

        let mut fsm = PullStateMachine::new();
        fsm.set_on_mode_change(callback);
        fsm.transition_to(PullMode::Chain);

        let log = callback_log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0], (PullMode::Normal, PullMode::Chain));
    }

    #[test]
    fn test_pull_mode_copy() {
        let mode1 = PullMode::Chain;
        let mode2 = mode1;
        assert_eq!(mode1, mode2);
    }
}

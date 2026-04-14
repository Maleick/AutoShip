use serde::Deserialize;
use textquest_common::combat::CombatStatus;
use textquest_common::nav::NavStatus;
use textquest_common::soul::IdleBehaviorType;
use textquest_common::types::GameState;

/// Configuration for when soul actions should be suppressed based on game state.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SuppressionRules {
    /// Suppress all chat during navigation (including in-combat responses)
    pub suppress_chat_during_navigation: bool,
    /// Suppress all chat during zone transitions
    pub suppress_chat_during_zone_change: bool,
    /// Suppress idle behaviors during combat
    pub suppress_idle_during_combat: bool,
    /// Suppress idle behaviors during navigation
    pub suppress_idle_during_navigation: bool,
    /// Suppress idle behaviors during zone transitions
    pub suppress_idle_during_zone_change: bool,
    /// Suppress movement-heavy idle behaviors during casting
    pub suppress_movement_idle_during_casting: bool,
    /// Suppress idle behaviors during looting
    pub suppress_idle_during_looting: bool,
    /// Allow only combat reaction responses during active combat
    pub combat_reactions_only_when_engaging: bool,
}

impl Default for SuppressionRules {
    fn default() -> Self {
        Self {
            suppress_chat_during_navigation: true,
            suppress_chat_during_zone_change: true,
            suppress_idle_during_combat: true,
            suppress_idle_during_navigation: true,
            suppress_idle_during_zone_change: true,
            suppress_movement_idle_during_casting: true,
            suppress_idle_during_looting: true,
            combat_reactions_only_when_engaging: true,
        }
    }
}

/// Extracted game state context relevant to soul action suppression.
#[derive(Debug, Clone, Copy)]
pub struct GameStateContext {
    pub in_combat: bool,
    pub is_engaging: bool,
    pub is_casting: bool,
    pub is_navigating: bool,
    pub is_looting: bool,
    pub is_zone_changing: bool,
}

impl GameStateContext {
    /// Extract suppression context from game state.
    #[must_use]
    pub fn from_game_state(state: &GameState) -> Self {
        let in_combat = !matches!(state.combat_status, CombatStatus::Idle);
        let is_engaging = matches!(state.combat_status, CombatStatus::Engaging { .. });
        let is_casting = matches!(state.combat_status, CombatStatus::Casting { .. });
        let is_navigating = !matches!(state.nav_status, NavStatus::Idle | NavStatus::Arrived);
        let is_looting = false; // TODO: Add looting state to GameState when loot tracking is implemented
        let is_zone_changing = false; // TODO: Add zone change detection when zone tracking is improved

        Self {
            in_combat,
            is_engaging,
            is_casting,
            is_navigating,
            is_looting,
            is_zone_changing,
        }
    }
}

impl SuppressionRules {
    /// Check if chat should be suppressed given the current context.
    #[must_use]
    pub fn should_suppress_chat(&self, ctx: GameStateContext) -> bool {
        (self.suppress_chat_during_navigation && ctx.is_navigating)
            || (self.suppress_chat_during_zone_change && ctx.is_zone_changing)
    }

    /// Check if idle should be suppressed given the current context.
    #[must_use]
    pub fn should_suppress_idle(&self, ctx: GameStateContext) -> bool {
        (self.suppress_idle_during_combat && ctx.in_combat)
            || (self.suppress_idle_during_navigation && ctx.is_navigating)
            || (self.suppress_idle_during_zone_change && ctx.is_zone_changing)
            || (self.suppress_idle_during_looting && ctx.is_looting)
    }

    /// Check if a specific idle behavior should be suppressed due to casting.
    /// Movement-heavy behaviors are suppressed; stationary ones are OK.
    #[must_use]
    pub fn should_suppress_behavior_for_casting(
        &self,
        behavior: &IdleBehaviorType,
        ctx: GameStateContext,
    ) -> bool {
        if !self.suppress_movement_idle_during_casting || !ctx.is_casting {
            return false;
        }

        // Behaviors that require movement
        matches!(
            behavior,
            IdleBehaviorType::Wander
                | IdleBehaviorType::Fish
                | IdleBehaviorType::VendorBrowse
                | IdleBehaviorType::RandomJump
        )
    }

    /// Check if combat reaction should be restricted to only engagement mode.
    #[must_use]
    pub fn should_restrict_combat_reactions(&self, ctx: GameStateContext) -> bool {
        self.combat_reactions_only_when_engaging && ctx.in_combat && !ctx.is_engaging
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::types::SpawnData;

    fn make_game_state(client_id: u32) -> GameState {
        GameState {
            client_id,
            local_player: Some(SpawnData {
                displayed_name: "TestChar".to_string(),
                name: "TestChar".to_string(),
                level: 60,
                ..SpawnData::default()
            }),
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "test".into(),
            zone_long_name: "Test Zone".into(),
            actual_version: None,
        }
    }

    #[test]
    fn suppression_rules_default() {
        let rules = SuppressionRules::default();
        assert!(rules.suppress_chat_during_navigation);
        assert!(rules.suppress_idle_during_combat);
    }

    #[test]
    fn game_state_context_idle_state() {
        let mut state = make_game_state(1);
        state.nav_status = NavStatus::Idle;
        state.combat_status = CombatStatus::Idle;

        let ctx = GameStateContext::from_game_state(&state);
        assert!(!ctx.in_combat);
        assert!(!ctx.is_navigating);
        assert!(!ctx.is_casting);
        assert!(!ctx.is_engaging);
    }

    #[test]
    fn game_state_context_in_combat() {
        let mut state = make_game_state(1);
        state.combat_status = CombatStatus::Engaging { target_id: 42 };

        let ctx = GameStateContext::from_game_state(&state);
        assert!(ctx.in_combat);
        assert!(ctx.is_engaging);
        assert!(!ctx.is_casting);
    }

    #[test]
    fn game_state_context_casting() {
        let mut state = make_game_state(1);
        state.combat_status = CombatStatus::Casting {
            spell_slot: 0,
            target_id: 42,
        };

        let ctx = GameStateContext::from_game_state(&state);
        assert!(ctx.in_combat);
        assert!(ctx.is_casting);
        assert!(!ctx.is_engaging);
    }

    #[test]
    fn game_state_context_navigating() {
        let mut state = make_game_state(1);
        state.nav_status = NavStatus::Moving {
            waypoint_index: 0,
            waypoint_count: 10,
            distance_remaining: 50.0,
        };

        let ctx = GameStateContext::from_game_state(&state);
        assert!(ctx.is_navigating);
        assert!(!ctx.in_combat);
    }

    #[test]
    fn game_state_context_arrived_is_not_navigating() {
        let mut state = make_game_state(1);
        state.nav_status = NavStatus::Arrived;

        let ctx = GameStateContext::from_game_state(&state);
        assert!(!ctx.is_navigating);
    }

    #[test]
    fn suppress_chat_during_navigation() {
        let rules = SuppressionRules::default();
        let mut state = make_game_state(1);
        state.nav_status = NavStatus::Moving {
            waypoint_index: 0,
            waypoint_count: 10,
            distance_remaining: 50.0,
        };

        let ctx = GameStateContext::from_game_state(&state);
        assert!(rules.should_suppress_chat(ctx));
    }

    #[test]
    fn suppress_idle_during_combat() {
        let rules = SuppressionRules::default();
        let mut state = make_game_state(1);
        state.combat_status = CombatStatus::Engaging { target_id: 42 };

        let ctx = GameStateContext::from_game_state(&state);
        assert!(rules.should_suppress_idle(ctx));
    }

    #[test]
    fn suppress_idle_during_navigation() {
        let rules = SuppressionRules::default();
        let mut state = make_game_state(1);
        state.nav_status = NavStatus::Moving {
            waypoint_index: 0,
            waypoint_count: 10,
            distance_remaining: 50.0,
        };

        let ctx = GameStateContext::from_game_state(&state);
        assert!(rules.should_suppress_idle(ctx));
    }

    #[test]
    fn dont_suppress_chat_when_idle() {
        let rules = SuppressionRules::default();
        let state = make_game_state(1);

        let ctx = GameStateContext::from_game_state(&state);
        assert!(!rules.should_suppress_chat(ctx));
    }

    #[test]
    fn dont_suppress_idle_when_idle() {
        let rules = SuppressionRules::default();
        let state = make_game_state(1);

        let ctx = GameStateContext::from_game_state(&state);
        assert!(!rules.should_suppress_idle(ctx));
    }

    #[test]
    fn suppress_wander_during_casting() {
        let rules = SuppressionRules::default();
        let mut state = make_game_state(1);
        state.combat_status = CombatStatus::Casting {
            spell_slot: 0,
            target_id: 42,
        };

        let ctx = GameStateContext::from_game_state(&state);
        assert!(rules.should_suppress_behavior_for_casting(&IdleBehaviorType::Wander, ctx));
        assert!(rules.should_suppress_behavior_for_casting(&IdleBehaviorType::Fish, ctx));
        assert!(rules.should_suppress_behavior_for_casting(&IdleBehaviorType::RandomJump, ctx));
    }

    #[test]
    fn dont_suppress_sit_during_casting() {
        let rules = SuppressionRules::default();
        let mut state = make_game_state(1);
        state.combat_status = CombatStatus::Casting {
            spell_slot: 0,
            target_id: 42,
        };

        let ctx = GameStateContext::from_game_state(&state);
        // Sit is stationary, shouldn't be suppressed
        assert!(!rules.should_suppress_behavior_for_casting(&IdleBehaviorType::Sit, ctx));
        assert!(!rules.should_suppress_behavior_for_casting(&IdleBehaviorType::LoreChatter, ctx));
    }

    #[test]
    fn restrict_combat_reactions_when_not_engaging() {
        let rules = SuppressionRules::default();
        let mut state = make_game_state(1);
        state.combat_status = CombatStatus::OnGcd;

        let ctx = GameStateContext::from_game_state(&state);
        assert!(rules.should_restrict_combat_reactions(ctx));
    }

    #[test]
    fn allow_combat_reactions_when_engaging() {
        let rules = SuppressionRules::default();
        let mut state = make_game_state(1);
        state.combat_status = CombatStatus::Engaging { target_id: 42 };

        let ctx = GameStateContext::from_game_state(&state);
        assert!(!rules.should_restrict_combat_reactions(ctx));
    }

    #[test]
    fn allow_chat_when_idle() {
        let rules = SuppressionRules::default();
        let state = make_game_state(1);

        let ctx = GameStateContext::from_game_state(&state);
        assert!(!rules.should_suppress_chat(ctx));
    }

    #[test]
    fn disable_suppression_via_config() {
        let rules = SuppressionRules {
            suppress_chat_during_navigation: false,
            ..SuppressionRules::default()
        };

        let mut state = make_game_state(1);
        state.nav_status = NavStatus::Moving {
            waypoint_index: 0,
            waypoint_count: 10,
            distance_remaining: 50.0,
        };

        let ctx = GameStateContext::from_game_state(&state);
        assert!(!rules.should_suppress_chat(ctx));
    }

    #[test]
    fn suppress_multiple_conditions_or() {
        let rules = SuppressionRules::default();
        let mut state = make_game_state(1);
        state.nav_status = NavStatus::Moving {
            waypoint_index: 0,
            waypoint_count: 10,
            distance_remaining: 50.0,
        };
        state.combat_status = CombatStatus::Engaging { target_id: 42 };

        let ctx = GameStateContext::from_game_state(&state);
        // Should suppress idle due to BOTH combat AND navigation
        assert!(rules.should_suppress_idle(ctx));
    }
}

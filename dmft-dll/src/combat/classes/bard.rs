use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Bard strategy: delegates song rotation to EQ's built-in `/melody` command.
///
/// Instead of a custom twist engine, we issue `/melody <slot1> <slot2> ...`
/// on engage and `/melody` (no args) to stop. The EQ client handles all
/// song cycling automatically.
pub struct BardStrategy {
    class_id: u8,
    /// True while melody is running (between engage and disengage).
    melody_active: bool,
}

impl BardStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            melody_active: false,
        }
    }

    /// Build the `/melody <slot> <slot> ...` command string from configured spells.
    fn melody_command(config_spells: &[SpellEntry]) -> String {
        if config_spells.is_empty() {
            return "/melody".to_string();
        }
        let slots: Vec<String> = config_spells.iter().map(|s| s.slot.to_string()).collect();
        format!("/melody {}", slots.join(" "))
    }
}

impl ClassStrategy for BardStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Bards assist the MA when in combat.
        if ctx.in_combat {
            ctx.target.map(|t| t.spawn_id)
        } else {
            None
        }
    }

    fn select_spell(&self, _ctx: &CombatContext) -> Option<SpellEntry> {
        // Melody handles song rotation automatically — nothing to cast.
        None
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        // Guard: don't issue bare `/melody` when no songs are configured —
        // EQ treats `/melody` (no args) as a stop/toggle command.
        if ctx.config.spells.is_empty() {
            tracing::warn!("Bard on_engage: no spells configured, skipping /melody");
            return;
        }

        let cmd = Self::melody_command(&ctx.config.spells);
        tracing::info!(
            command = %cmd,
            songs = ctx.config.spells.len(),
            "Bard engaging — starting /melody"
        );
        crate::eq::slash_command(&cmd);
        self.melody_active = true;
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        // If combat ended, stop melody.
        if !ctx.in_combat && self.melody_active {
            tracing::info!("Bard disengaging — stopping /melody");
            crate::eq::slash_command("/melody");
            self.melody_active = false;
        }
    }

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::Support
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dmft_common::types::SpawnData;

    fn make_spell(id: i32, name: &str, slot: u8) -> SpellEntry {
        SpellEntry {
            slot,
            spell_id: id,
            name: name.to_string(),
            min_mana_pct: 0.0,
            priority: 1,
            is_aoe: false,
        }
    }

    #[test]
    fn melody_command_with_songs() {
        let spells = vec![
            make_spell(100, "Selo's", 1),
            make_spell(101, "Chant", 2),
            make_spell(102, "Anthem", 5),
        ];
        let cmd = BardStrategy::melody_command(&spells);
        assert_eq!(cmd, "/melody 1 2 5");
    }

    #[test]
    fn melody_command_empty() {
        let cmd = BardStrategy::melody_command(&[]);
        assert_eq!(cmd, "/melody");
    }

    #[test]
    fn select_spell_returns_none() {
        let bard = BardStrategy::new(8);
        let player = SpawnData::default();
        let config = dmft_common::combat::CombatConfig {
            spells: vec![make_spell(100, "Selo's", 1)],
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
        };
        assert!(bard.select_spell(&ctx).is_none());
    }

    #[test]
    fn bard_role_is_support() {
        let bard = BardStrategy::new(8);
        assert!(matches!(bard.role(), CombatRole::Support));
    }

    #[test]
    fn on_engage_activates_melody() {
        let mut bard = BardStrategy::new(8);
        assert!(!bard.melody_active);

        let player = SpawnData::default();
        let config = dmft_common::combat::CombatConfig {
            spells: vec![make_spell(100, "Selo's", 1), make_spell(101, "Chant", 2)],
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
        };
        bard.on_engage(&ctx);
        assert!(bard.melody_active);
    }

    #[test]
    fn on_action_complete_stops_melody_when_combat_ends() {
        let mut bard = BardStrategy::new(8);
        bard.melody_active = true;

        let player = SpawnData::default();
        let config = dmft_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 100,
            in_combat: false, // Combat ended
        };
        bard.on_action_complete(&ctx);
        assert!(!bard.melody_active);
    }

    #[test]
    fn on_action_complete_keeps_melody_during_combat() {
        let mut bard = BardStrategy::new(8);
        bard.melody_active = true;

        let player = SpawnData::default();
        let config = dmft_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 100,
            in_combat: true,
        };
        bard.on_action_complete(&ctx);
        assert!(bard.melody_active); // Still active during combat
    }

    #[test]
    fn on_engage_empty_spells_does_not_activate_melody() {
        let mut bard = BardStrategy::new(8);
        assert!(!bard.melody_active);

        let player = SpawnData::default();
        let config = dmft_common::combat::CombatConfig {
            spells: vec![], // No spells configured
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
        };
        bard.on_engage(&ctx);
        // melody_active should remain false — no /melody command issued
        assert!(!bard.melody_active);
    }

    #[test]
    fn class_id_is_8() {
        let bard = BardStrategy::new(8);
        assert_eq!(bard.class_id(), 8);
    }

    #[test]
    fn bard_class_id_custom() {
        let bard = BardStrategy::new(42);
        assert_eq!(bard.class_id(), 42);
    }

    #[test]
    fn melody_cleanup_on_flee_full_cycle() {
        // Verify the full engage → disengage cycle cleans up melody state.
        let mut bard = BardStrategy::new(8);
        assert!(!bard.melody_active);

        let player = SpawnData::default();
        let config = dmft_common::combat::CombatConfig {
            spells: vec![make_spell(100, "Selo's", 1), make_spell(101, "Chant", 2)],
            ..Default::default()
        };

        // Engage — melody starts
        let engage_ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
        };
        bard.on_engage(&engage_ctx);
        assert!(
            bard.melody_active,
            "melody should be active after on_engage"
        );

        // Combat ends — melody should stop
        let disengage_ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 50,
            in_combat: false,
        };
        bard.on_action_complete(&disengage_ctx);
        assert!(
            !bard.melody_active,
            "melody should be inactive after combat ends"
        );
    }

    #[test]
    fn melody_command_empty_edge_case_returns_bare_command() {
        // Edge case: empty spell list should produce bare /melody with no slots.
        let cmd = BardStrategy::melody_command(&[]);
        assert_eq!(cmd, "/melody");
    }
}

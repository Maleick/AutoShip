use textquest_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext, PetAction};

/// Shaman strategy: hybrid healer/slower/DoT. Prioritizes slow on new targets,
/// heals when group HP is low, `DoTs` otherwise.
/// EQ class ID: 10
pub struct ShamanStrategy {
    class_id: u8,
    /// Track if current target has been slowed (resets on target change)
    target_slowed: bool,
    last_target_id: u32,
}

impl ShamanStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            target_slowed: false,
            last_target_id: 0,
        }
    }

    fn afflicted_member(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::prioritized_afflicted_member(ctx).map(|(spawn_id, _)| spawn_id)
    }

    fn find_cure_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        let cures: Vec<&SpellEntry> = ctx
            .config
            .spells
            .iter()
            .filter(|s| strategy::is_standard_cure_spell(s))
            .filter(|s| mana_pct >= s.min_mana_pct)
            .collect();
        if strategy::afflicted_member_count(ctx) > 1
            && let Some(group_cure) = cures
                .iter()
                .copied()
                .filter(|s| strategy::is_group_cure_spell(s))
                .max_by_key(|s| s.priority)
        {
            return Some(group_cure.clone());
        }
        cures.into_iter().max_by_key(|s| s.priority).cloned()
    }

    fn cure_target(&self, ctx: &CombatContext) -> Option<u32> {
        if strategy::afflicted_member_count(ctx) > 1
            && self
                .find_cure_spell(ctx)
                .is_some_and(|spell| strategy::is_group_cure_spell(&spell))
        {
            return Some(ctx.player.spawn_id);
        }
        self.afflicted_member(ctx)
    }
}

impl ClassStrategy for ShamanStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        if let Some(afflicted_id) = self.cure_target(ctx) {
            return Some(afflicted_id);
        }

        // If someone needs healing, target them
        if let Some((heal_target, hp)) = strategy::lowest_hp_member(ctx)
            && hp < 70.0
        {
            return Some(heal_target);
        }
        // Otherwise target the mob (for slow/DoT)
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Priority 0: Cure detrimental effects (shaman is the premier curer)
        if strategy::afflicted_member_count(ctx) > 0 {
            if let Some(cure) = self.find_cure_spell(ctx) {
                return Some(cure);
            }
        }

        // Priority 1: Emergency heal (group member below 40%)
        if let Some((_, hp)) = strategy::lowest_hp_member(ctx)
            && hp < 40.0
        {
            return ctx
                .config
                .spells
                .iter()
                .filter(|s| s.name.contains("Heal") || s.name.contains("heal"))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned();
        }

        // Priority 2: Slow on unslowed target
        if !self.target_slowed
            && let Some(slow) = ctx
                .config
                .spells
                .iter()
                .filter(|s| {
                    s.name.contains("Slow") || s.name.contains("slow") || s.name.contains("Turgur")
                })
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(slow);
        }

        // Priority 3: Heal if anyone below 70%
        if let Some((_, hp)) = strategy::lowest_hp_member(ctx)
            && hp < 70.0
        {
            return ctx
                .config
                .spells
                .iter()
                .filter(|s| s.name.contains("Heal") || s.name.contains("heal"))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned();
        }

        // Priority 4: DoT / nuke (exclude heals, slows, and debuffs)
        ctx.config
            .spells
            .iter()
            .filter(|s| {
                !s.name.contains("Heal")
                    && !s.name.contains("heal")
                    && !s.name.contains("Slow")
                    && !s.name.contains("slow")
                    && !s.name.contains("Turgur")
                    && !s.name.contains("Malo")
            })
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn pet_action(&self, ctx: &CombatContext) -> Option<PetAction> {
        strategy::pet_attack_action(ctx)
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        let new_target = ctx.target.map_or(0, |t| t.spawn_id);
        if new_target != self.last_target_id {
            self.target_slowed = false;
            self.last_target_id = new_target;
        }
        if let Some(target) = ctx.target {
            tracing::info!(
                target_id = target.spawn_id,
                target_name = %target.name,
                slowed = self.target_slowed,
                "Shaman engaging"
            );
        }
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        // Only reset slow tracking when out of combat (target died / disengage).
        // During combat, on_engage handles new-target resets. Resetting here
        // unconditionally caused the shaman to re-cast slow every GCD cycle.
        if !ctx.in_combat {
            self.target_slowed = false;
            self.last_target_id = 0;
        }
    }

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::Healer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::strategy::GroupMemberState;
    use textquest_common::combat::{AssistMode, CombatConfig};

    fn test_config_with_spells() -> CombatConfig {
        CombatConfig {
            role: CombatRole::Healer,
            pull_method: None,
            assist_mode: AssistMode::AssistTrain,
            mana_floor: 20.0,
            aoe_threshold: 3,
            spells: vec![
                SpellEntry {
                    slot: 1,
                    spell_id: 0,
                    name: "Turgur's Insects".into(),
                    min_mana_pct: 30.0,
                    priority: 10,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 2,
                    spell_id: 0,
                    name: "Greater Healing".into(),
                    min_mana_pct: 20.0,
                    priority: 8,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 3,
                    spell_id: 0,
                    name: "Envenomed Bolt".into(),
                    min_mana_pct: 25.0,
                    priority: 5,
                    is_aoe: false,
                },
            ],
            holyshit_rules: vec![],
            disciplines: vec![],
            target_scan: textquest_common::combat::TargetScanConfig::default(),
        }
    }

    #[test]
    fn shaman_class_id() {
        let shaman = ShamanStrategy::new(10);
        assert_eq!(shaman.class_id(), 10);
    }

    #[test]
    fn shaman_role_is_healer() {
        let shaman = ShamanStrategy::new(10);
        assert_eq!(shaman.role(), CombatRole::Healer);
    }

    #[test]
    fn shaman_prioritizes_slow_on_new_target() {
        let shaman = ShamanStrategy::new(10);
        let config = test_config_with_spells();
        let player = textquest_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
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
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        let spell = shaman.select_spell(&ctx);
        assert!(spell.is_some());
        assert!(spell.unwrap().name.contains("Turgur"));
    }

    #[test]
    fn shaman_heals_when_group_low() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true; // already slowed
        let config = test_config_with_spells();
        let player = textquest_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![GroupMemberState {
            spawn_id: 1,
            hp_pct: 30.0,
            mana_pct: 50.0,
            class_id: 1,
            is_dead: false,
            name: "Warrior".into(),
            has_detrimental: false,
        }];
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        let spell = shaman.select_spell(&ctx);
        assert!(spell.is_some());
        assert!(spell.unwrap().name.contains("Heal"));
    }

    #[test]
    fn shaman_dots_when_slowed_and_healthy() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        let config = test_config_with_spells();
        let player = textquest_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
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
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        let spell = shaman.select_spell(&ctx);
        assert!(spell.is_some());
        assert!(spell.unwrap().name.contains("Envenomed"));
    }

    #[test]
    fn shaman_aoe_threshold() {
        let shaman = ShamanStrategy::new(10);
        assert_eq!(shaman.aoe_threshold(), 3);
    }

    #[test]
    fn shaman_should_assist() {
        let shaman = ShamanStrategy::new(10);
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        assert!(shaman.should_assist(&ctx));
    }

    #[test]
    fn shaman_select_target_heal_when_low() {
        let shaman = ShamanStrategy::new(10);
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let target = textquest_common::types::SpawnData {
            spawn_id: 99,
            ..Default::default()
        };
        let group = vec![GroupMemberState {
            spawn_id: 42,
            hp_pct: 50.0, // below 70%
            mana_pct: 100.0,
            class_id: 1,
            is_dead: false,
            name: String::new(),
            has_detrimental: false,
        }];
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &group,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        assert_eq!(shaman.select_target(&ctx), Some(42));
    }

    #[test]
    fn shaman_select_target_mob_when_healthy() {
        let shaman = ShamanStrategy::new(10);
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let target = textquest_common::types::SpawnData {
            spawn_id: 99,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        assert_eq!(shaman.select_target(&ctx), Some(99));
    }

    #[test]
    fn shaman_select_target_afflicted_member_before_heal_or_dps() {
        let shaman = ShamanStrategy::new(10);
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let target = textquest_common::types::SpawnData {
            spawn_id: 99,
            ..Default::default()
        };
        let group = vec![
            GroupMemberState {
                spawn_id: 42,
                hp_pct: 85.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: String::new(),
                has_detrimental: true,
            },
            GroupMemberState {
                spawn_id: 43,
                hp_pct: 50.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: String::new(),
                has_detrimental: false,
            },
        ];
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &group,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        assert_eq!(shaman.select_target(&ctx), Some(42));
    }

    #[test]
    fn shaman_group_cure_targets_self_for_multiple_afflicted_members() {
        let shaman = ShamanStrategy::new(10);
        let player = textquest_common::types::SpawnData {
            spawn_id: 10,
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let group = vec![
            GroupMemberState {
                spawn_id: 42,
                hp_pct: 85.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: String::new(),
                has_detrimental: true,
            },
            GroupMemberState {
                spawn_id: 43,
                hp_pct: 50.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: String::new(),
                has_detrimental: true,
            },
        ];
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Cure Disease".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 2,
                    spell_id: 101,
                    name: "Radiant Cure".into(),
                    min_mana_pct: 10.0,
                    priority: 1,
                    is_aoe: true,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &group,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };

        assert_eq!(shaman.select_spell(&ctx).unwrap().name, "Radiant Cure");
        assert_eq!(shaman.select_target(&ctx), Some(10));
    }

    #[test]
    fn shaman_on_engage_resets_slow_on_new_target() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        shaman.last_target_id = 1;
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let new_target = textquest_common::types::SpawnData {
            spawn_id: 2,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&new_target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        shaman.on_engage(&ctx);
        assert!(!shaman.target_slowed);
        assert_eq!(shaman.last_target_id, 2);
    }

    #[test]
    fn shaman_on_engage_keeps_slow_same_target() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        shaman.last_target_id = 5;
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let target = textquest_common::types::SpawnData {
            spawn_id: 5,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        shaman.on_engage(&ctx);
        assert!(shaman.target_slowed);
    }

    #[test]
    fn shaman_no_spells_returns_none() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        assert!(shaman.select_spell(&ctx).is_none());
    }
}

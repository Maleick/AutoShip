use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Paladin strategy: off-tank + healer hybrid, stuns, heals, undead nukes.
/// EQ class ID: 3
pub struct PaladinStrategy {
    class_id: u8,
}

impl PaladinStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for PaladinStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::assist_target(ctx)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Priority 1: Heal group members below 50% HP
        for member in ctx.group_members {
            if member.hp_pct < 50.0
                && let Some(heal) = ctx
                    .config
                    .spells
                    .iter()
                    .filter(|s| {
                        s.name.contains("Heal")
                            || s.name.contains("Light")
                            || s.name.contains("Cure")
                    })
                    .filter(|s| mana_pct >= s.min_mana_pct)
                    .max_by_key(|s| s.priority)
                    .cloned()
            {
                return Some(heal);
            }
        }

        // Priority 2: Stun (interrupt casters, generate aggro)
        if let Some(stun) = ctx
            .config
            .spells
            .iter()
            .filter(|s| s.name.contains("Stun") || s.name.contains("Force"))
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
        {
            return Some(stun);
        }

        // Priority 3: Highest priority spell from config
        ctx.config
            .spells
            .iter()
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false // Paladin tanks, doesn't assist
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Paladin");
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        if !ctx.in_combat {
            strategy::melee_on_disengage();
        }
    }

    fn aoe_threshold(&self) -> u8 {
        2
    }

    fn role(&self) -> CombatRole {
        CombatRole::OffTank
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paladin_class_id() {
        let pal = PaladinStrategy::new(3);
        assert_eq!(pal.class_id(), 3);
    }

    #[test]
    fn paladin_role_is_off_tank() {
        let pal = PaladinStrategy::new(3);
        assert_eq!(pal.role(), CombatRole::OffTank);
    }

    #[test]
    fn paladin_does_not_assist() {
        let pal = PaladinStrategy::new(3);
        let config = dmft_common::combat::CombatConfig::default();
        let player = dmft_common::types::SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
        };
        assert!(!pal.should_assist(&ctx));
    }

    #[test]
    fn paladin_aoe_threshold() {
        let pal = PaladinStrategy::new(3);
        assert_eq!(pal.aoe_threshold(), 2);
    }

    #[test]
    fn paladin_heal_priority_when_group_low_hp() {
        let pal = PaladinStrategy::new(3);
        let mut player = dmft_common::types::SpawnData::default();
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = dmft_common::combat::CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Holy Light".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Stun of Valor".into(),
                    min_mana_pct: 10.0,
                    priority: 5,
                    is_aoe: false,
                },
            ],
            ..dmft_common::combat::CombatConfig::default()
        };
        let group = vec![crate::combat::strategy::GroupMemberState {
            spawn_id: 1,
            hp_pct: 30.0, // below 50%
            mana_pct: 100.0,
            class_id: 1,
        }];
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &group,
            config: &config,
            tick: 0,
            in_combat: true,
        };
        let spell = pal.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Holy Light"); // heal priority
    }

    #[test]
    fn paladin_stun_when_group_healthy() {
        let pal = PaladinStrategy::new(3);
        let mut player = dmft_common::types::SpawnData::default();
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = dmft_common::combat::CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Holy Light".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Stun".into(),
                    min_mana_pct: 10.0,
                    priority: 5,
                    is_aoe: false,
                },
            ],
            ..dmft_common::combat::CombatConfig::default()
        };
        let group = vec![crate::combat::strategy::GroupMemberState {
            spawn_id: 1,
            hp_pct: 90.0, // healthy
            mana_pct: 100.0,
            class_id: 1,
        }];
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &group,
            config: &config,
            tick: 0,
            in_combat: true,
        };
        let spell = pal.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Stun"); // stun priority when no one needs healing
    }

    #[test]
    fn paladin_fallback_to_any_spell() {
        let pal = PaladinStrategy::new(3);
        let mut player = dmft_common::types::SpawnData::default();
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = dmft_common::combat::CombatConfig {
            spells: vec![dmft_common::combat::SpellEntry {
                slot: 1,
                spell_id: 300,
                name: "Undead Nuke".into(),
                min_mana_pct: 10.0,
                priority: 3,
                is_aoe: false,
            }],
            ..dmft_common::combat::CombatConfig::default()
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
        let spell = pal.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Undead Nuke"); // fallback
    }

    #[test]
    fn paladin_no_spells_returns_none() {
        let pal = PaladinStrategy::new(3);
        let player = dmft_common::types::SpawnData::default();
        let config = dmft_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
        };
        assert!(pal.select_spell(&ctx).is_none());
    }
}

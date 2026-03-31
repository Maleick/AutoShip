use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Druid strategy: hybrid healer/nuker/snarer. Prioritizes heals when group is hurt,
/// snare on runners, DoTs/nukes otherwise.
/// EQ class ID: 6
pub struct DruidStrategy {
    class_id: u8,
}

impl DruidStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn lowest_hp_member(&self, ctx: &CombatContext) -> Option<(u32, f32)> {
        ctx.group_members
            .iter()
            .filter(|m| m.hp_pct < 100.0 && m.hp_pct > 0.0)
            .min_by(|a, b| {
                a.hp_pct
                    .partial_cmp(&b.hp_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|m| (m.spawn_id, m.hp_pct))
    }
}

impl ClassStrategy for DruidStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        if let Some((heal_target, hp)) = self.lowest_hp_member(ctx)
            && hp < 65.0
        {
            return Some(heal_target);
        }
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Priority 1: Emergency heal
        if let Some((_, hp)) = self.lowest_hp_member(ctx)
            && hp < 45.0
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

        // Priority 2: Snare on low-HP mob (fleeing prevention)
        if let Some(target) = ctx.target
            && target.hp_pct() < 20.0
            && let Some(snare) = ctx
                .config
                .spells
                .iter()
                .filter(|s| {
                    s.name.contains("Snare")
                        || s.name.contains("snare")
                        || s.name.contains("Ensnare")
                })
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(snare);
        }

        // Priority 3: Heal if group member below 65%
        if let Some((_, hp)) = self.lowest_hp_member(ctx)
            && hp < 65.0
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

        // Priority 4: Nuke/DoT
        ctx.config
            .spells
            .iter()
            .filter(|s| {
                !s.name.contains("Heal")
                    && !s.name.contains("heal")
                    && !s.name.contains("Snare")
                    && !s.name.contains("snare")
                    && !s.name.contains("Ensnare")
            })
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
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

    #[test]
    fn druid_class_id() {
        let druid = DruidStrategy::new(6);
        assert_eq!(druid.class_id(), 6);
    }

    #[test]
    fn druid_role_is_healer() {
        let druid = DruidStrategy::new(6);
        assert_eq!(druid.role(), CombatRole::Healer);
    }

    #[test]
    fn druid_should_assist() {
        let druid = DruidStrategy::new(6);
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
        assert!(druid.should_assist(&ctx));
    }

    #[test]
    fn druid_aoe_threshold() {
        let druid = DruidStrategy::new(6);
        assert_eq!(druid.aoe_threshold(), 3);
    }

    #[test]
    fn druid_emergency_heal_below_45() {
        let druid = DruidStrategy::new(6);
        let mut player = dmft_common::types::SpawnData::default();
        player.mana_current = 8000;
        player.mana_max = 10000;
        let config = dmft_common::combat::CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Greater Healing".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Starfire".into(),
                    min_mana_pct: 10.0,
                    priority: 15,
                    is_aoe: false,
                },
            ],
            ..dmft_common::combat::CombatConfig::default()
        };
        let group = vec![crate::combat::strategy::GroupMemberState {
            spawn_id: 1,
            hp_pct: 30.0,
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
        let spell = druid.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Greater Healing");
    }

    #[test]
    fn druid_snare_on_fleeing_mob() {
        let druid = DruidStrategy::new(6);
        let mut player = dmft_common::types::SpawnData::default();
        player.mana_current = 8000;
        player.mana_max = 10000;
        let mut target = dmft_common::types::SpawnData::default();
        target.hp_current = 1000;
        target.hp_max = 10000; // 10% HP
        let config = dmft_common::combat::CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Ensnare".into(),
                    min_mana_pct: 10.0,
                    priority: 8,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Starfire".into(),
                    min_mana_pct: 10.0,
                    priority: 15,
                    is_aoe: false,
                },
            ],
            ..dmft_common::combat::CombatConfig::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
        };
        let spell = druid.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Ensnare");
    }

    #[test]
    fn druid_heal_between_45_and_65() {
        let druid = DruidStrategy::new(6);
        let mut player = dmft_common::types::SpawnData::default();
        player.mana_current = 8000;
        player.mana_max = 10000;
        let config = dmft_common::combat::CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Greater Healing".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Starfire".into(),
                    min_mana_pct: 10.0,
                    priority: 15,
                    is_aoe: false,
                },
            ],
            ..dmft_common::combat::CombatConfig::default()
        };
        let group = vec![crate::combat::strategy::GroupMemberState {
            spawn_id: 1,
            hp_pct: 55.0,
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
        let spell = druid.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Greater Healing");
    }

    #[test]
    fn druid_nukes_when_group_healthy() {
        let druid = DruidStrategy::new(6);
        let mut player = dmft_common::types::SpawnData::default();
        player.mana_current = 8000;
        player.mana_max = 10000;
        let config = dmft_common::combat::CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Greater Healing".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Starfire".into(),
                    min_mana_pct: 10.0,
                    priority: 15,
                    is_aoe: false,
                },
            ],
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
        let spell = druid.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Starfire");
    }

    #[test]
    fn druid_select_target_heal_when_low() {
        let druid = DruidStrategy::new(6);
        let config = dmft_common::combat::CombatConfig::default();
        let player = dmft_common::types::SpawnData::default();
        let target = dmft_common::types::SpawnData {
            spawn_id: 99,
            ..Default::default()
        };
        let group = vec![crate::combat::strategy::GroupMemberState {
            spawn_id: 42,
            hp_pct: 40.0,
            mana_pct: 100.0,
            class_id: 1,
        }];
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &group,
            config: &config,
            tick: 0,
            in_combat: true,
        };
        assert_eq!(druid.select_target(&ctx), Some(42)); // heal target, not mob
    }

    #[test]
    fn druid_select_target_mob_when_healthy() {
        let druid = DruidStrategy::new(6);
        let config = dmft_common::combat::CombatConfig::default();
        let player = dmft_common::types::SpawnData::default();
        let target = dmft_common::types::SpawnData {
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
        };
        assert_eq!(druid.select_target(&ctx), Some(99)); // mob target
    }

    #[test]
    fn druid_no_spells_returns_none() {
        let druid = DruidStrategy::new(6);
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
        assert!(druid.select_spell(&ctx).is_none());
    }
}

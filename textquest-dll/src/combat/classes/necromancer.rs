use textquest_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Necromancer strategy: DoT-focused DPS with pet, lifetap sustain, feign death escape.
/// EQ class ID: 11
pub struct NecromancerStrategy {
    class_id: u8,
}

impl NecromancerStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for NecromancerStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        let hp_pct = ctx.player.hp_pct();

        // Priority 1: Lifetap when HP is low (self-sustain)
        if hp_pct < 50.0
            && let Some(tap) = ctx
                .config
                .spells
                .iter()
                .filter(|s| {
                    s.name.contains("Tap")
                        || s.name.contains("tap")
                        || s.name.contains("Drain")
                        || s.name.contains("Leech")
                })
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(tap);
        }

        // Priority 2: DoTs (necro's bread and butter)
        if let Some(dot) = ctx
            .config
            .spells
            .iter()
            .filter(|s| {
                s.name.contains("Venom")
                    || s.name.contains("Poison")
                    || s.name.contains("Darkness")
                    || s.name.contains("Plague")
                    || s.name.contains("Disease")
                    || s.name.contains("Fire")
            })
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
        {
            return Some(dot);
        }

        // Priority 3: Any available spell
        ctx.config
            .spells
            .iter()
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn aoe_threshold(&self) -> u8 {
        255 // Necros don't AoE (DoT-based)
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsRanged
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::types::SpawnData;

    #[test]
    fn necro_class_id() {
        let necro = NecromancerStrategy::new(11);
        assert_eq!(necro.class_id(), 11);
    }

    #[test]
    fn necro_role_is_ranged_dps() {
        let necro = NecromancerStrategy::new(11);
        assert_eq!(necro.role(), CombatRole::DpsRanged);
    }

    #[test]
    fn necro_no_aoe() {
        let necro = NecromancerStrategy::new(11);
        assert_eq!(necro.aoe_threshold(), 255);
    }

    #[test]
    fn necro_always_assists() {
        let necro = NecromancerStrategy::new(11);
        let player = SpawnData::default();
        let config = textquest_common::combat::CombatConfig::default();
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
        };
        assert!(necro.should_assist(&ctx));
    }

    #[test]
    fn necro_select_target_returns_current_target() {
        let necro = NecromancerStrategy::new(11);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 99,
            ..SpawnData::default()
        };
        let config = textquest_common::combat::CombatConfig::default();
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
        };
        assert_eq!(necro.select_target(&ctx), Some(99));
    }

    #[test]
    fn necro_lifetap_priority_when_low_hp() {
        let necro = NecromancerStrategy::new(11);
        let mut player = SpawnData::default();
        player.hp_current = 3000;
        player.hp_max = 10000; // 30% HP
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = textquest_common::combat::CombatConfig {
            spells: vec![
                textquest_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Lifetap".into(),
                    min_mana_pct: 10.0,
                    priority: 5,
                    is_aoe: false,
                },
                textquest_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Venom of Solusek".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
            ],
            ..textquest_common::combat::CombatConfig::default()
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
        };
        let spell = necro.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Lifetap"); // lifetap priority at low HP
    }

    #[test]
    fn necro_dot_priority_when_hp_healthy() {
        let necro = NecromancerStrategy::new(11);
        let mut player = SpawnData::default();
        player.hp_current = 9000;
        player.hp_max = 10000; // 90% HP
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = textquest_common::combat::CombatConfig {
            spells: vec![
                textquest_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Lifetap".into(),
                    min_mana_pct: 10.0,
                    priority: 5,
                    is_aoe: false,
                },
                textquest_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Venom of Solusek".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                textquest_common::combat::SpellEntry {
                    slot: 3,
                    spell_id: 300,
                    name: "Nuke".into(),
                    min_mana_pct: 10.0,
                    priority: 3,
                    is_aoe: false,
                },
            ],
            ..textquest_common::combat::CombatConfig::default()
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
        };
        let spell = necro.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Venom of Solusek"); // DoT priority
    }

    #[test]
    fn necro_fallback_when_no_dots_or_taps() {
        let necro = NecromancerStrategy::new(11);
        let mut player = SpawnData::default();
        player.hp_current = 9000;
        player.hp_max = 10000;
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = textquest_common::combat::CombatConfig {
            spells: vec![textquest_common::combat::SpellEntry {
                slot: 1,
                spell_id: 100,
                name: "Nuke".into(),
                min_mana_pct: 10.0,
                priority: 3,
                is_aoe: false,
            }],
            ..textquest_common::combat::CombatConfig::default()
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
        };
        let spell = necro.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Nuke"); // fallback
    }

    #[test]
    fn necro_no_spells_returns_none() {
        let necro = NecromancerStrategy::new(11);
        let player = SpawnData::default();
        let config = textquest_common::combat::CombatConfig::default();
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
        };
        assert!(necro.select_spell(&ctx).is_none());
    }

    #[test]
    fn necro_mana_filter_skips_expensive_spells() {
        let necro = NecromancerStrategy::new(11);
        let mut player = SpawnData::default();
        player.hp_current = 9000;
        player.hp_max = 10000;
        player.mana_current = 500;
        player.mana_max = 10000; // 5% mana
        let config = textquest_common::combat::CombatConfig {
            spells: vec![textquest_common::combat::SpellEntry {
                slot: 1,
                spell_id: 100,
                name: "Venom of Fire".into(),
                min_mana_pct: 20.0, // requires 20%, we have 5%
                priority: 10,
                is_aoe: false,
            }],
            ..textquest_common::combat::CombatConfig::default()
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
        };
        assert!(necro.select_spell(&ctx).is_none());
    }
}

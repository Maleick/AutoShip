use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Shadow Knight strategy: off-tank with lifetap DPS, disease/poison DoTs, snare.
/// EQ class ID: 5
pub struct ShadowKnightStrategy {
    class_id: u8,
}

impl ShadowKnightStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for ShadowKnightStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::assist_target(ctx)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        let hp_pct = ctx.player.hp_pct();

        // Priority 1: Lifetap when HP is low
        if hp_pct < 60.0
            && let Some(tap) = ctx
                .config
                .spells
                .iter()
                .filter(|s| {
                    s.name.contains("Tap")
                        || s.name.contains("tap")
                        || s.name.contains("Leech")
                        || s.name.contains("Drain")
                })
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(tap);
        }

        // Priority 2: Snare on fleeing mob
        if let Some(target) = ctx.target
            && target.hp_pct() < 15.0
            && let Some(snare) = ctx
                .config
                .spells
                .iter()
                .filter(|s| s.name.contains("Snare") || s.name.contains("Darkness"))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(snare);
        }

        // Priority 3: Disease/poison DoTs and nukes
        ctx.config
            .spells
            .iter()
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false // SK tanks, doesn't assist
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Shadow Knight");
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
    fn sk_class_id() {
        let sk = ShadowKnightStrategy::new(5);
        assert_eq!(sk.class_id(), 5);
    }

    #[test]
    fn sk_role_is_off_tank() {
        let sk = ShadowKnightStrategy::new(5);
        assert_eq!(sk.role(), CombatRole::OffTank);
    }

    #[test]
    fn sk_does_not_assist() {
        let sk = ShadowKnightStrategy::new(5);
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
            ch_chain_slot: None,
        };
        assert!(!sk.should_assist(&ctx));
    }

    #[test]
    fn sk_aoe_threshold() {
        let sk = ShadowKnightStrategy::new(5);
        assert_eq!(sk.aoe_threshold(), 2);
    }

    #[test]
    fn sk_lifetap_priority_when_low_hp() {
        let sk = ShadowKnightStrategy::new(5);
        let mut player = dmft_common::types::SpawnData::default();
        player.hp_current = 4000;
        player.hp_max = 10000; // 40% HP
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = dmft_common::combat::CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Lifetap".into(),
                    min_mana_pct: 10.0,
                    priority: 5,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Disease Cloud".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
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
            ch_chain_slot: None,
        };
        let spell = sk.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Lifetap"); // lifetap priority at low HP
    }

    #[test]
    fn sk_snare_on_fleeing_mob() {
        let sk = ShadowKnightStrategy::new(5);
        let mut player = dmft_common::types::SpawnData::default();
        player.hp_current = 9000;
        player.hp_max = 10000; // 90% HP (healthy, no lifetap)
        player.mana_current = 5000;
        player.mana_max = 10000;
        let mut target = dmft_common::types::SpawnData::default();
        target.hp_current = 1000;
        target.hp_max = 10000; // 10% HP (below 15%, fleeing)
        let config = dmft_common::combat::CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Lifetap".into(),
                    min_mana_pct: 10.0,
                    priority: 5,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Darkness Snare".into(),
                    min_mana_pct: 10.0,
                    priority: 8,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 3,
                    spell_id: 300,
                    name: "Nuke".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
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
            ch_chain_slot: None,
        };
        let spell = sk.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Darkness Snare"); // snare on fleeing mob
    }

    #[test]
    fn sk_fallback_to_generic() {
        let sk = ShadowKnightStrategy::new(5);
        let mut player = dmft_common::types::SpawnData::default();
        player.hp_current = 9000;
        player.hp_max = 10000;
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = dmft_common::combat::CombatConfig {
            spells: vec![dmft_common::combat::SpellEntry {
                slot: 1,
                spell_id: 300,
                name: "Nuke".into(),
                min_mana_pct: 10.0,
                priority: 10,
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
            ch_chain_slot: None,
        };
        let spell = sk.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Nuke");
    }

    #[test]
    fn sk_no_spells_returns_none() {
        let sk = ShadowKnightStrategy::new(5);
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
            ch_chain_slot: None,
        };
        assert!(sk.select_spell(&ctx).is_none());
    }
}

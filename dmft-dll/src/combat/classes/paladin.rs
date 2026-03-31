use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Paladin strategy: off-tank + healer hybrid, stuns, cures, heals, undead nukes.
///
/// Priority order (MQ2-style cascade):
/// 1. Stun (interrupt, aggro)
/// 2. Cure disease/poison (Paladins get Cure Disease at level 6, Cure Poison at 22)
/// 3. Emergency heal (< 40% HP)
/// 4. Moderate heal (< 60% HP)
/// 5. Nuke (undead DD, general)
///
/// EQ class ID: 3
pub struct PaladinStrategy {
    class_id: u8,
}

impl PaladinStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

fn is_cure_spell(s: &SpellEntry) -> bool {
    let name = s.name.to_lowercase();
    name.contains("cure") || name.contains("purify") || name.contains("remove")
}

fn is_heal_spell(s: &SpellEntry) -> bool {
    let name = s.name.to_lowercase();
    (name.contains("heal") || name.contains("light")) && !is_cure_spell(s)
}

fn is_stun_spell(s: &SpellEntry) -> bool {
    let name = s.name.to_lowercase();
    name.contains("stun") || name.contains("force")
}

impl ClassStrategy for PaladinStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // If someone needs healing, target them
        if let Some((heal_target, hp)) = strategy::lowest_hp_member(ctx)
            && hp < 60.0
        {
            return Some(heal_target);
        }
        // Otherwise use assist target for stuns/nukes
        strategy::assist_target(ctx)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Priority 1: Stun (interrupt casters, generate aggro)
        // Skip stun if a group member needs healing — select_target will have
        // returned a friendly heal target, so casting a hostile stun on them
        // makes no sense and causes a stun/heal oscillation loop.
        let needs_heal = strategy::lowest_hp_member(ctx).is_some_and(|(_, hp)| hp < 60.0);
        if ctx.in_combat && !needs_heal {
            if let Some(stun) = ctx
                .config
                .spells
                .iter()
                .filter(|s| is_stun_spell(s))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
            {
                return Some(stun);
            }
        }

        // Priority 2: Cure disease/poison on afflicted group member
        let has_afflicted = ctx
            .group_members
            .iter()
            .any(|m| !m.is_dead && m.has_detrimental);
        if has_afflicted {
            if let Some(cure) = ctx
                .config
                .spells
                .iter()
                .filter(|s| is_cure_spell(s))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
            {
                return Some(cure);
            }
        }

        // Priority 3: Emergency heal (< 40% HP)
        if let Some((_, hp)) = strategy::lowest_hp_member(ctx)
            && hp < 40.0
        {
            return ctx
                .config
                .spells
                .iter()
                .filter(|s| is_heal_spell(s))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned();
        }

        // Priority 4: Moderate heal (< 60% HP)
        if let Some((_, hp)) = strategy::lowest_hp_member(ctx)
            && hp < 60.0
        {
            return ctx
                .config
                .spells
                .iter()
                .filter(|s| is_heal_spell(s))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .min_by_key(|s| s.priority) // use efficient (low rank) heal for moderate damage
                .cloned();
        }

        // Priority 5: Nuke (undead DD, general damage — exclude heals, cures, stuns)
        ctx.config
            .spells
            .iter()
            .filter(|s| !is_heal_spell(s) && !is_cure_spell(s) && !is_stun_spell(s))
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
            ch_chain_slot: None,
        };
        assert!(!pal.should_assist(&ctx));
    }

    #[test]
    fn paladin_aoe_threshold() {
        let pal = PaladinStrategy::new(3);
        assert_eq!(pal.aoe_threshold(), 2);
    }

    fn test_config_with_spells() -> dmft_common::combat::CombatConfig {
        use dmft_common::combat::{AssistMode, CombatConfig};
        CombatConfig {
            role: CombatRole::OffTank,
            pull_method: None,
            assist_mode: AssistMode::AssistTrain,
            mana_floor: 20.0,
            aoe_threshold: 2,
            spells: vec![
                SpellEntry {
                    slot: 1,
                    spell_id: 0,
                    name: "Stun".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 2,
                    spell_id: 0,
                    name: "Cure Disease".into(),
                    min_mana_pct: 10.0,
                    priority: 8,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 3,
                    spell_id: 0,
                    name: "Light Healing".into(),
                    min_mana_pct: 10.0,
                    priority: 5,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 4,
                    spell_id: 0,
                    name: "Greater Healing".into(),
                    min_mana_pct: 15.0,
                    priority: 7,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 5,
                    spell_id: 0,
                    name: "Holy Might".into(),
                    min_mana_pct: 20.0,
                    priority: 6,
                    is_aoe: false,
                },
            ],
            holyshit_rules: vec![],
            disciplines: vec![],
        }
    }

    fn make_member(
        spawn_id: u32,
        hp_pct: f32,
        has_detrimental: bool,
    ) -> crate::combat::strategy::GroupMemberState {
        crate::combat::strategy::GroupMemberState {
            spawn_id,
            hp_pct,
            mana_pct: 100.0,
            class_id: 3,
            is_dead: false,
            name: format!("Player{spawn_id}"),
            has_detrimental,
        }
    }

    #[test]
    fn paladin_cures_afflicted_member() {
        let pal = PaladinStrategy::new(3);
        let config = test_config_with_spells();
        let player = dmft_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(1, 90.0, true)]; // afflicted but healthy
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: false, // out of combat, no stun
            ch_chain_slot: None,
        };
        let spell = pal.select_spell(&ctx);
        assert!(spell.is_some());
        assert!(spell.unwrap().name.contains("Cure"));
    }

    #[test]
    fn paladin_stuns_before_cure_in_combat() {
        let pal = PaladinStrategy::new(3);
        let config = test_config_with_spells();
        let player = dmft_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(1, 90.0, true)];
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
        };
        let spell = pal.select_spell(&ctx);
        assert!(spell.is_some());
        assert!(spell.unwrap().name.contains("Stun"));
    }

    #[test]
    fn paladin_emergency_heal_below_40() {
        let pal = PaladinStrategy::new(3);
        let config = test_config_with_spells();
        let player = dmft_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(1, 30.0, false)];
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
        };
        let spell = pal.select_spell(&ctx);
        assert!(spell.is_some());
        // Emergency uses max priority heal → Greater Healing (priority 7)
        assert!(spell.unwrap().name.contains("Healing"));
    }

    #[test]
    fn paladin_nukes_when_healthy_no_affliction() {
        let pal = PaladinStrategy::new(3);
        let config = test_config_with_spells();
        let player = dmft_common::types::SpawnData {
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
            in_combat: false,
            ch_chain_slot: None,
        };
        let spell = pal.select_spell(&ctx);
        assert!(spell.is_some());
        assert_eq!(spell.unwrap().name, "Holy Might");
    }
}

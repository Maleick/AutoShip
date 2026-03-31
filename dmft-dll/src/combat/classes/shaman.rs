use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Shaman strategy: hybrid healer/slower/DoT. Prioritizes slow on new targets,
/// heals when group HP is low, DoTs otherwise.
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

impl ClassStrategy for ShamanStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // If someone needs healing, target them
        if let Some((heal_target, hp)) = self.lowest_hp_member(ctx)
            && hp < 70.0
        {
            return Some(heal_target);
        }
        // Otherwise target the mob (for slow/DoT)
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Priority 1: Emergency heal (group member below 40%)
        if let Some((_, hp)) = self.lowest_hp_member(ctx)
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
        if let Some((_, hp)) = self.lowest_hp_member(ctx)
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

    fn on_engage(&mut self, ctx: &CombatContext) {
        let new_target = ctx.target.map(|t| t.spawn_id).unwrap_or(0);
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

    fn on_action_complete(&mut self, _ctx: &CombatContext) {
        self.target_slowed = false;
        self.last_target_id = 0;
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
    use dmft_common::combat::{AssistMode, CombatConfig};

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
            in_combat: true,
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
        let player = dmft_common::types::SpawnData {
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
        }];
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
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
            in_combat: true,
        };
        let spell = shaman.select_spell(&ctx);
        assert!(spell.is_some());
        assert!(spell.unwrap().name.contains("Envenomed"));
    }
}

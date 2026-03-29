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
            .min_by(|a, b| a.hp_pct.partial_cmp(&b.hp_pct).unwrap_or(std::cmp::Ordering::Equal))
            .map(|m| (m.spawn_id, m.hp_pct))
    }
}

impl ClassStrategy for DruidStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        if let Some((heal_target, hp)) = self.lowest_hp_member(ctx)
            && hp < 65.0 {
                return Some(heal_target);
            }
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Priority 1: Emergency heal
        if let Some((_, hp)) = self.lowest_hp_member(ctx)
            && hp < 45.0 {
                return ctx.config.spells.iter()
                    .filter(|s| s.name.contains("Heal") || s.name.contains("heal"))
                    .filter(|s| mana_pct >= s.min_mana_pct)
                    .max_by_key(|s| s.priority)
                    .cloned();
            }

        // Priority 2: Snare on low-HP mob (fleeing prevention)
        if let Some(target) = ctx.target
            && target.hp_pct() < 20.0
                && let Some(snare) = ctx.config.spells.iter()
                    .filter(|s| s.name.contains("Snare") || s.name.contains("snare")
                             || s.name.contains("Ensnare"))
                    .filter(|s| mana_pct >= s.min_mana_pct)
                    .max_by_key(|s| s.priority)
                    .cloned()
                {
                    return Some(snare);
                }

        // Priority 3: Heal if group member below 65%
        if let Some((_, hp)) = self.lowest_hp_member(ctx)
            && hp < 65.0 {
                return ctx.config.spells.iter()
                    .filter(|s| s.name.contains("Heal") || s.name.contains("heal"))
                    .filter(|s| mana_pct >= s.min_mana_pct)
                    .max_by_key(|s| s.priority)
                    .cloned();
            }

        // Priority 4: Nuke/DoT
        ctx.config.spells.iter()
            .filter(|s| !s.name.contains("Heal") && !s.name.contains("heal")
                     && !s.name.contains("Snare") && !s.name.contains("snare")
                     && !s.name.contains("Ensnare"))
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, _ctx: &CombatContext) {}
    fn on_kill(&mut self, _ctx: &CombatContext) {}

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
}

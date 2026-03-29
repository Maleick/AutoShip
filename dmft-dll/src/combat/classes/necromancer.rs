use dmft_common::combat::{CombatRole, SpellEntry};

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
            && let Some(tap) = ctx.config.spells.iter()
                .filter(|s| s.name.contains("Tap") || s.name.contains("tap")
                         || s.name.contains("Drain") || s.name.contains("Leech"))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
            {
                return Some(tap);
            }

        // Priority 2: DoTs (necro's bread and butter)
        if let Some(dot) = ctx.config.spells.iter()
            .filter(|s| s.name.contains("Venom") || s.name.contains("Poison")
                     || s.name.contains("Darkness") || s.name.contains("Plague")
                     || s.name.contains("Disease") || s.name.contains("Fire"))
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
        {
            return Some(dot);
        }

        // Priority 3: Any available spell
        ctx.config.spells.iter()
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
        255 // Necros don't AoE (DoT-based)
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsRanged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

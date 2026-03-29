use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Magician strategy: pet-based DPS + nukes. Pet management via /pet commands.
/// EQ class ID: 13
pub struct MagicianStrategy {
    class_id: u8,
}

impl MagicianStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for MagicianStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        ctx.config.spells.iter()
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
        CombatRole::DpsRanged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mage_class_id() {
        let mage = MagicianStrategy::new(13);
        assert_eq!(mage.class_id(), 13);
    }

    #[test]
    fn mage_role_is_ranged_dps() {
        let mage = MagicianStrategy::new(13);
        assert_eq!(mage.role(), CombatRole::DpsRanged);
    }
}

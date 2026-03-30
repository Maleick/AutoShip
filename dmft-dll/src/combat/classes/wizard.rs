use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Wizard strategy: pure nuke DPS. Highest priority spell available, mana-aware.
/// EQ class ID: 5
pub struct WizardStrategy {
    class_id: u8,
}

impl WizardStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for WizardStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
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
    fn wizard_class_id() {
        let wiz = WizardStrategy::new(5);
        assert_eq!(wiz.class_id(), 5);
    }

    #[test]
    fn wizard_role_is_ranged_dps() {
        let wiz = WizardStrategy::new(5);
        assert_eq!(wiz.role(), CombatRole::DpsRanged);
    }
}
